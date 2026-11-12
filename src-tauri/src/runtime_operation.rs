//! One reservation shared by Android mod writes and native launch acceptance.
use std::sync::atomic::{AtomicBool, Ordering};

static RESERVED: AtomicBool = AtomicBool::new(false);

pub(crate) fn is_reserved() -> bool {
    RESERVED.load(Ordering::Acquire)
}

pub(crate) struct RuntimeOperation;

pub(crate) fn reserve_runtime_operation() -> Result<RuntimeOperation, String> {
    RESERVED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| "Another HQ operation is in progress. Try again shortly.".to_string())?;
    Ok(RuntimeOperation)
}

impl Drop for RuntimeOperation {
    fn drop(&mut self) {
        RESERVED.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    #[test]
    fn concurrent_launch_and_mod_reservations_have_one_owner_and_drop_releases_it() {
        let start = Arc::new(Barrier::new(3));
        let acquired = Arc::new(Barrier::new(3));
        let threads: Vec<_> = (0..2)
            .map(|_| {
                let start = start.clone();
                let acquired = acquired.clone();
                std::thread::spawn(move || {
                    start.wait();
                    let reservation = reserve_runtime_operation();
                    let won = reservation.is_ok();
                    acquired.wait();
                    drop(reservation);
                    won
                })
            })
            .collect();
        start.wait();
        acquired.wait();
        let owners = threads
            .into_iter()
            .map(|thread| usize::from(thread.join().unwrap()))
            .sum::<usize>();
        assert_eq!(owners, 1);
        assert!(reserve_runtime_operation().is_ok());
        assert!(!is_reserved());
    }
}
