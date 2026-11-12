# Playable modded Android and version support

User requests modded gameplay, English mobile UI matching desktop, and versions beyond v81. Continue autonomously; retain existing data, account login, desktop APIs and the common responsive UI.

1. Diagnose with session logs and single-feature toggles. Isolated PathfindingLib 2.4.1 startup stall; Android excludes it and dependent PathfindingLagFix. Other choices/config restored. Modded v81 ship rendering, joystick movement and camera drag verified.
2. English: all own Android UI/native runtime/control strings and tests use existing desktop terms (HQ Run, version/install, Mods, Settings). No i18n dependency.
3. Version architecture: source HQ's trusted version→depot catalog, use authenticated internal Steam's fixed download_depot command for old content, store separately and launch via supported internal Proton/game override. Verify exact supported runtime path before implementing; never overwrite current Steam install/saves to simulate a version.
4. Focused tests: version catalog/path/command constraints and incomplete downloads, version-specific mods/preflight/Tab metadata; browser loading/error/install/select/locks and English controls. Review before final APK update, then actual device install + modded gameplay and a second version.

No separate downloader authentication or copied credentials. If Steam refuses an old manifest, surface its actual error. No partial success claims for gameplay.
