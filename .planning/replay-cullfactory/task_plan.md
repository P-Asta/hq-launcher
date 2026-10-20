# Replay / CullFactory restoration

1. Inspect reset state (done: HEAD 403e114).
2. Restore tests, warning, and conflict enforcement (done).
3. Rust/JS/build/browser verification and review (done).

Full Rust suite: 124 passed. JS warning/compatibility/bridge checks and frontend build passed. Browser tests use mocked Tauri. Actual game runtime not tested.
