//! Standalone Windows helper used under Wine/Proton to inject `hq_overlay.dll`
//! into a game process.
//!
//! The launcher itself runs as a native Linux process and therefore cannot
//! call `CreateRemoteThread`/`OpenProcess` against the Wine game process.
//! Instead it spawns this helper (a small x64 Windows binary) inside the same
//! Wine prefix. The helper mirrors the injection logic of
//! `inject_native_overlay_dll_into_process` in `lib.rs`:
//!
//!   OpenProcess -> VirtualAllocEx -> WriteProcessMemory (wide DLL path)
//!   -> resolve LoadLibraryW (relocated for the target process)
//!   -> CreateRemoteThread -> wait + verify the exit code is non-null.
//!
//! Usage: `hq-inject-helper.exe <pid> <dll_path>`
//! Exit code 0 on success, non-zero on failure (an error message is printed
//! to stderr).

use std::ffi::OsString;
use std::os::windows::ffi::OsStrExt;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, MODULEENTRY32W, TH32CS_SNAPMODULE,
    TH32CS_SNAPMODULE32,
};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
use windows_sys::Win32::System::Memory::{
    VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE,
};
use windows_sys::Win32::System::Threading::{
    CreateRemoteThread, GetExitCodeThread, GetCurrentProcessId, OpenProcess, WaitForSingleObject,
    PROCESS_CREATE_THREAD, PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION, PROCESS_VM_READ,
    PROCESS_VM_WRITE,
};

const LOAD_TIMEOUT_MS: u32 = 15_000;

struct ProcessModule {
    name: String,
    base: usize,
    size: usize,
}

fn enum_process_modules(pid: u32) -> Result<Vec<ProcessModule>, String> {
    let snapshot =
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(format!(
            "CreateToolhelp32Snapshot failed for pid {pid} (error {})",
            unsafe { GetLastError() }
        ));
    }
    let result = (|| {
        let mut entry = MODULEENTRY32W {
            dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
            ..Default::default()
        };
        if unsafe { Module32FirstW(snapshot, &mut entry) } == 0 {
            return Err(format!(
                "Module32FirstW failed for pid {pid} (error {})",
                unsafe { GetLastError() }
            ));
        }
        let mut modules = Vec::new();
        loop {
            let name_len = entry
                .szModule
                .iter()
                .position(|value| *value == 0)
                .unwrap_or(entry.szModule.len());
            modules.push(ProcessModule {
                name: String::from_utf16_lossy(&entry.szModule[..name_len]),
                base: entry.modBaseAddr as usize,
                size: entry.modBaseSize as usize,
            });
            if unsafe { Module32NextW(snapshot, &mut entry) } == 0 {
                break;
            }
        }
        Ok(modules)
    })();
    unsafe {
        CloseHandle(snapshot);
    }
    result
}

/// `LoadLibraryW` lives in kernel32, which is ASLR-relocated per process. Under
/// Wine the kernel32 base is shared across processes in the same prefix, but we
/// relocate defensively in case a future Wine build diverges.
fn remote_load_library_w(pid: u32, local_address: usize) -> Result<usize, String> {
    let local_modules = enum_process_modules(unsafe { GetCurrentProcessId() })?;
    let local_module = local_modules
        .iter()
        .find(|module| local_address >= module.base && local_address - module.base < module.size)
        .ok_or_else(|| {
            format!("could not identify local module for address 0x{local_address:x}")
        })?;
    let remote_modules = enum_process_modules(pid)?;
    let remote_module = remote_modules
        .iter()
        .find(|module| module.name.eq_ignore_ascii_case(&local_module.name))
        .ok_or_else(|| {
            format!(
                "target process {pid} does not contain module {}",
                local_module.name
            )
        })?;
    let offset = local_address.checked_sub(local_module.base).ok_or_else(|| {
        format!(
            "local address 0x{local_address:x} below module {} base",
            local_module.name
        )
    })?;
    if offset >= local_module.size {
        return Err(format!(
            "local address offset 0x{offset:x} outside module {} size",
            local_module.name
        ));
    }
    remote_module
        .base
        .checked_add(offset)
        .ok_or_else(|| "relocated address overflow".to_string())
}

fn inject(pid: u32, dll_path: &str) -> Result<(), String> {
    let dll_path_wide: Vec<u16> = OsString::from(dll_path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let process = unsafe {
        OpenProcess(
            PROCESS_CREATE_THREAD
                | PROCESS_QUERY_INFORMATION
                | PROCESS_VM_OPERATION
                | PROCESS_VM_WRITE
                | PROCESS_VM_READ,
            0,
            pid,
        )
    };
    if process.is_null() {
        return Err(format!(
            "OpenProcess failed for pid {pid} (error {})",
            unsafe { GetLastError() }
        ));
    }

    let result = (|| {
        let alloc_size = dll_path_wide.len() * std::mem::size_of::<u16>();
        let remote_memory = unsafe {
            VirtualAllocEx(
                process,
                null_mut(),
                alloc_size,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        };
        if remote_memory.is_null() {
            return Err(format!(
                "VirtualAllocEx failed (error {})",
                unsafe { GetLastError() }
            ));
        }

        let write_ok = unsafe {
            windows_sys::Win32::System::Diagnostics::Debug::WriteProcessMemory(
                process,
                remote_memory,
                dll_path_wide.as_ptr().cast(),
                alloc_size,
                null_mut(),
            )
        };
        if write_ok == 0 {
            unsafe {
                VirtualFreeEx(process, remote_memory, 0, MEM_RELEASE);
            }
            return Err(format!(
                "WriteProcessMemory failed (error {})",
                unsafe { GetLastError() }
            ));
        }

        let kernel32 = unsafe { GetModuleHandleA(b"kernel32.dll\0".as_ptr()) };
        if kernel32.is_null() {
            unsafe {
                VirtualFreeEx(process, remote_memory, 0, MEM_RELEASE);
            }
            return Err(format!(
                "GetModuleHandleA(kernel32.dll) failed (error {})",
                unsafe { GetLastError() }
            ));
        }
        let local_load_library_w = unsafe { GetProcAddress(kernel32, b"LoadLibraryW\0".as_ptr()) };
        let Some(local_load_library_w) = local_load_library_w else {
            unsafe {
                VirtualFreeEx(process, remote_memory, 0, MEM_RELEASE);
            }
            return Err("GetProcAddress(LoadLibraryW) failed".to_string());
        };
        let remote_load_library_w =
            match remote_load_library_w(pid, local_load_library_w as usize) {
                Ok(address) => address,
                Err(error) => {
                    unsafe {
                        VirtualFreeEx(process, remote_memory, 0, MEM_RELEASE);
                    }
                    return Err(error);
                }
            };

        let remote_thread = unsafe {
            CreateRemoteThread(
                process,
                null(),
                0,
                Some(std::mem::transmute(remote_load_library_w)),
                remote_memory,
                0,
                null_mut(),
            )
        };
        if remote_thread.is_null() {
            unsafe {
                VirtualFreeEx(process, remote_memory, 0, MEM_RELEASE);
            }
            return Err(format!(
                "CreateRemoteThread failed (error {})",
                unsafe { GetLastError() }
            ));
        }

        let wait_result = unsafe { WaitForSingleObject(remote_thread, LOAD_TIMEOUT_MS) };
        if wait_result != WAIT_OBJECT_0 {
            unsafe {
                CloseHandle(remote_thread);
            }
            return Err(format!(
                "LoadLibraryW did not complete within {LOAD_TIMEOUT_MS} ms (wait result {wait_result})"
            ));
        }

        let mut load_result = 0u32;
        let exit_code_ok = unsafe { GetExitCodeThread(remote_thread, &mut load_result) };
        unsafe {
            CloseHandle(remote_thread);
            VirtualFreeEx(process, remote_memory, 0, MEM_RELEASE);
        }
        if exit_code_ok == 0 {
            return Err(format!(
                "GetExitCodeThread failed (error {})",
                unsafe { GetLastError() }
            ));
        }
        if load_result == 0 {
            return Err(format!(
                "LoadLibraryW rejected {dll_path}; verify it is a valid x64 DLL"
            ));
        }
        Ok(())
    })();

    unsafe {
        CloseHandle(process);
    }
    result
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: {} <pid> <dll_path>", args.first().map(String::as_str).unwrap_or("hq-inject-helper"));
        std::process::exit(2);
    }

    let pid: u32 = match args[1].parse() {
        Ok(value) => value,
        Err(_) => {
            eprintln!("invalid pid: {}", args[1]);
            std::process::exit(2);
        }
    };
    let dll_path = &args[2];

    if let Err(error) = inject(pid, dll_path) {
        eprintln!("inject failed: {error}");
        std::process::exit(1);
    }
}
