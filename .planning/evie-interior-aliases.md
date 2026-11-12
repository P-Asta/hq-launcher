# Evie interior aliases

- [x] Inspect existing Evie layout and interior normalization.
- [x] Add regression coverage for requested sheet names and verify failure.
- [x] Add the three missing aliases, preserving existing normalization.
- [x] Run Rust tests and review the scoped diff.

Scope: `src-tauri/src/lcstats_autosheet/layouts/evieautosheet.rs` only, plus this plan.

Regression test failed as expected: `Level3ButCoolRenewed` was written unchanged to column J instead of `Exp Mines`.

Verification: `cargo test --manifest-path src-tauri/Cargo.toml` passed (143 unit tests; binary and doc tests also passed). Existing compiler warnings remain in unrelated modules. Scoped `git diff --check` passed. No live Google Sheets write was performed.
