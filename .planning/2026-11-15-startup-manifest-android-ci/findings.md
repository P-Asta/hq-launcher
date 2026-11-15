# Findings
- HEAD 820cf9d is v1.15.0. Initially only unrelated untracked investigation/screenshot files exist.
- Prior investigation reproduced Tokio starvation: mod_config.rs uses Condvar::wait inside async fetch. Five requests on four workers stalled beyond the 12-second HTTP timeout. v1.15.0 adds a startup practice-mod manifest consumer.
- Current cache is memory-only, TTL 60 seconds. All consumers share ModsConfig::load_manifest. RemoteManifest contains mods, practice mods, game manifests, chains and preset constraints.
- Stable/beta use release_channel::current. LCF appears in RUN_OPTIONS/RUN_MODE_VALUES and saved preferences.
- Existing android:build:debug prepares the pinned runtime and calls Tauri ARM64 --apk --ci. Existing GitHub workflow handles desktops only.
- Android uses JDK 17, SDK 36, Gradle 8.14.3 and NDK 28.2.13676358. These are installed locally.
- GitHub release 0.5.0 public DroidDeck-0.5.0.apk is 105925476 bytes with SHA-256 ab6a53e41b6fa715bd3f9048ada53d22d8535ad2978d97943d723cf60fa430b1, exactly matching the existing lock. CI needs no private donor secret.
- Context7 Tauri v2 documentation confirms ANDROID_HOME/NDK_HOME and aarch64 target selection.

- Broader root-cause search found the same Condvar wait in event_config.rs, also used during startup. Replace that wait with an async mutex as part of the startup fix; preserve 404-as-empty and stale in-memory fallback.

- Current tauri-build 2.5.6 src/mobile.rs generates ignored tauri.settings.gradle and app/tauri.build.gradle.kts during Cargo build, so CI recreates host-specific generated plugin paths from a clean checkout.
- Diomedes successful receipts record all resolved package versions; launch and HQ pin restoration verify actual metadata before proceeding. Review fixes preserve installed game/save/config files.

- Follow-up root cause: apply_diomedes_mod_states passed all installed packages as forced-disabled and UI hid all base rows. Now preserves base preferences, excluding VLog/HQoL/save-ui and additionally user-requested Replay/LCStatsTracker. LCF lacked mode affinity in lists; mark by identity and force-disable on other mode transitions.
