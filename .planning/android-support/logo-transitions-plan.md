# HQ logo transitions implementation plan

Goal: reuse HQ's existing logo across all launcher and internal runtime loading transitions. User confirmed this scope; game-owned Unity/menu artwork is outside it.

Architecture: one React HqLogo/HqLoading component and CSS motion, and one Compose logo using the exact same public/icon.svg copied by Gradle. Stage/progress readiness stays controlled by existing state. No new dependencies, game files, credentials, saves or runtime behavior changes.

- [x] Inspect actual entry points, bootstrap/progress components and native loading/pause overlay.
- [x] RED: hold Android startup IPC and require visible HQ logo; observed locator timeout before implementation.
- [x] Implement shared logo reveal (1.6s), 160ms content entrance, honest progress fill, reduced-motion static mode. Apply to frontend startup/preparation/task/update/login/runtime/settings indicators.
- [x] GREEN: actual Android fixture with startup release, progress and reduced-motion assertions; desktop responsive fixture passed.
- [x] Native: stage tracked source patches replacing the loading/pause mark, remove unrelated artwork/flood, fade loading onto the first real compositor frame. Preserve cancel/retry/share/close and progress.
- [x] Verify preparation and full APK build, browser visual checks, fresh source review, install without clearing phone data, inspect actual native loading transition and game handoff.

Review boundaries: empty/unknown/complete progress, errors and controls, no artificial timers waiting for animation, game overlay window stays transparent, disabled system animations, native first-frame exit and assets packaged from one source.

Ruling: keep existing bar/progress/error/cancel affordances and ordinary hover/focus states. Logo motion replaces screen-loading branding and busy indicators; do not insert a full-screen interstitial for every button or dialog.

Verification: Android startup RED→GREEN, full Android and actual desktop responsive browser fixtures, logo visual/progress fixture at320/412/915/1280px, reduced motion and delayed game/OBS transparency checks passed. Preparation15/15 and production desktop build passed. Full APK build passed after native incremental cache clean; final English APK build34s passed and update installed. Real device shows partial HQ reveal + English controls and reaches actual game Launch Mode. Native stop confirmation/return tested; final session stop requested. Steam self-update restarted one verification session; retry after update reached game normally. No user save/config/version choices changed. Pause/error presentations compiled/reviewed; those branches were not forced on device.
