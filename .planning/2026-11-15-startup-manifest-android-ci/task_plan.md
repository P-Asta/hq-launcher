# Startup recovery and Android CI Implementation Plan

Goal: Fix startup manifest stalls and persist a last-good manifest, hide LCF selection, and build installable Android APKs in GitHub Actions.
Architecture: Async manifest cache lock with per-channel JSON in application data. Reuse existing ARM64 wrapper and pinned runtime preparation.
Stack: React 19, Tauri CLI 2.10.1, Rust/Tokio, SDK 36, JDK 17, Yarn 3.6.4, Python 3.
Spec: User requests in this chat; findings.md records investigation evidence.

## Tasks
1. [x] Inspect startup, prior investigation and Android build prerequisites.
2. [x] Add failing manifest tests; fix blocking waits and persist validated downloads. Files: mod_config.rs, desktop/Android setup, Cargo.toml if needed. Run cargo test --locked --lib mod_config::tests.
3. [x] Hide LCF in RUN_OPTIONS and reject restored hidden selections. Verify other modes in Node and browser.
4. [x] Add .github/workflows/android.yml, pinned donor URL and docs/ANDROID.md instructions. Build ARM64 debug APK on push/PR/manual runs; upload APK and checksum.
5. [x] Run Rust/Node/frontend checks and local Android packaging, review changes independently, resolve material findings.

## Constraints and review focus
- Preserve invoke APIs, local development override, stable/beta isolation, cancellation and installation/save data.
- Aborted requests must release waiters; cancelled requests must not silently succeed from cache.
- HTTP/JSON failure must not overwrite good data; invalid disk cache must not count as success.
- Restored LCF preferences must select HQ safely.
- CI must build from a clean checkout without signing secrets and verify exact donor SHA-256/size.
- No new dependencies; no commit, push or publication requested.

## Execution decisions
- Work in the existing checkout; preserve unrelated untracked investigation and screenshot files.
- Keep the concise plan here, matching existing .planning conventions.
- Use the public DroidDeck 0.5.0 asset: its published digest and size exactly match the current lock.

## Errors / limits
- Initial searches referenced absent paths.rs/settings.rs; followed module structure instead.
- Initial plan rewrite via delete/add in one apply_patch was rejected; use direct text replacement for generated plan files.
- GitHub-hosted execution cannot be claimed from local validation alone.

## Added scope
- [x] Diomedes preset: exact user package IDs, latest mod releases, latest supported game version only, visible install progress; dedicated agent handles frontend/backend integration and LCF. User confirmed only old mod files are removed; game and saves are preserved.



## Follow-up: preserve base mods and remove lingering LCF
- [x] Add regressions for selective Diomedes disables and LCF affinity/hiding.
- [x] Apply narrow VLog/HQoL/save-ui/Replay/LCStatsTracker exclusions, preserve normal user preferences, disable legacy LCF across presets.
- [x] Run focused Rust/Node/browser checks and update documentation; full-suite native abort recorded in progress.md.

## Release v1.15.2
- [x] Confirm clean checkout, remote main and absence of the release tag; update application/package versions and release notes.
- [x] Verify version consistency, locked metadata, frontend build and regression tests (163 Rust tests passed).
- Publication: user authorized a release commit, annotated v1.15.2 and main/tag push. Execution and remote-ref verification are recorded in release-publish.log.

