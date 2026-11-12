# Android support

## Active follow-up: minimal HQ controls menu and new touch actions

HQ button opens only mouse sensitivity and key layout settings. Add Ctrl hold, Shift toggle, dedicated right-click and Space. Surface short tap becomes left-click; camera drags and multitouch stay intact. Preserve English UI, existing E/G/scroll/80+ Tab, runtime/mod/version behavior and saves. User confirmed key layout means on-screen positions and sizes.

1. [x] Inspect existing input model, overlay, lifecycle and HQ menu wiring.
2. [x] Focused JVM RED/GREEN for new actions, toggle persistence/release, left tap and sensitivity.
3. [x] Implement minimal native settings and persistent control layout; add/reflow buttons.
4. [x] Review, build/update APK and verify real game input/settings/lifecycle, including cold-start control selection regression.

## Active follow-up: HQ logo screen transitions

Use the existing HQ logo for launcher startup, screen-loading/installation states and the internal Android runtime loading screen. Preserve responsive layout, actual progress, errors/cancel/retry controls and game/session behavior. Default motion: quiet vertical logo reveal and short content fade; no minimum artificial loading delay. Reduced-motion/system animation-disabled users get a static logo. User confirmed all launcher and internal runtime screens; game-owned artwork stays outside this scope.

1. [x] Inspect actual desktop/Android entry points, loading components, native overlay and logo assets.
2. [x] Add shared logo/loading motion; apply to startup, preparation and task/status transitions on both launcher frontends.
3. [x] Replace Android runtime loading presentation with HQ branding, retaining operational controls.
4. [x] Browser/reduced-motion/regression verification, source review, APK build/update and native logo/English/game handoff/exit check. Final APK installed; selected v73 retained.

## Active follow-up: playable HQ mods, English mobile UI, other game versions

User explicitly requests all three. Preserve shared responsive presentation, desktop behavior and installed data.

1. [x] Isolate PathfindingLib2.4.1 startup stall; apply Android-only exclusion of it and dependent LagFix, with visible reason. Actual modded v81 LAN local-only new game, joystick movement and camera drag verified on phone.
2. [x] Change Android launcher and HQ native control/status strings to English, matching desktop terminology.
3. [x] Reuse HQ's verified manifest catalog to install/select other owned game versions on Android; protect per-version mods/config and existing game files. Actual managed v73 download/receipt/selection/mods/wrapper launch verified.
4. [x] Run focused regression checks/review, build and update phone, verify modded v81/v73 ship/input, managed download/selection and return to81 launch on device.

## Active correction: shared responsive HQ launcher UI

Use existing desktop workspace/toolbar/mod rows in Android, with narrow-width stacking and wrapping. Android detection selects runtime actions, never an unrelated visual design. See responsive-launcher-plan.md. Preserve ongoing native touch controls.

- [x] Share existing presentation and adapt small widths on desktop and Android.
- [x] Browser regression/resize checks, desktop build, Android APK update and phone inspection.
- [x] Fix verified native GUI input review findings and run focused JVM tests.

## Active follow-up: Amethyst-style Lethal touch controls

User specified AngelAuraMC/Amethyst-Android as the reference: movement joystick,
E interaction, G drop, Tab utility only on game v80+, wheel up/down, and its camera/click touch behavior.

1. [x] Inspect reference input code, license and HQ's native session layering; restore vanilla diagnostic config.
2. [x] Implement tested multi-touch keyboard/mouse state and native game overlay with no new dependency.
3. [x] Carry a verified, depot-keyed game version to native controls for the Tab floor; preserve setup controls.
4. [x] Review, build/update phone, and verify touch routing on the real game (v81/v73 joystick/camera; v73 tap scan).
5. [x] Isolate modded black startup; Android-only compatibility exclusion and actual gameplay verified. Multiplayer/voice remain untested.

Reference pin:330c6eae3164df64bdc4828e946a9e62cc5169e4, LGPL3. Read-only local checkout under ignored .android-runtime.local/Amethyst-Android.
Use original HQ implementation of the observed behavior, not Minecraft runtime/dependencies. Reference game taps are right-click within150ms, stationary hold at300ms is left-button held; menu taps are direct left click. Joystick emits8-direction WASD. Multi-touch input must release on cancel, focus loss and overlays.

## Current follow-up: game install/launch without Steam Deck UI

User wants HQ's Lethal Company installation and launch flow instead of the Steam
Deck-style client. Keep the internal runtime and mods. Root cause: HQ omitted
EXTRA_STEAM_UI, so the pinned engine defaults to -gamepadui/-steamdeck.

1. [x] Inspect existing runtime session options and launch scripts.
2. [x] Add a game-specific install action; force normal Steam UI for install and launch, with compact setup controls.
3. [ ] Verify UI/engine behavior, build and update the phone APK while retaining its runtime data.
4. [x] Identify observed missing atl.dll as an in-progress Proton update; allow Steam to complete its own update and verify restored files/mod initialization.
5. [x] Verify cold game launch and automatic game focus in the final APK; preserve normal installation UI.
6. [x] Diagnose modded black screen: PathfindingLib2.4.1 isolated and excluded with dependent LagFix. Original diagnostic configuration restored; actual modded ship verified.

Steam authentication and game installation confirmation remain necessary. The
user previously deferred login. Fresh device status now reports game v81 and HQ mods installed, so game launch is being tested; do not claim gameplay from installed state alone.

## Active follow-up: internal Linux/Steam and HQ mods

User superseded the external-app integration: HQ must own the Linux/Steam runtime
inside its APK/app data, install the owned game there and apply HQ mods. It must
work on the phone without a PC connection or separately installed DroidDeck app.
Keep the completed first milestone below as history; its scope exclusions no
longer bound the active work.

1. [x] Inspect pinned DroidDeck source, native inputs and module integration.
2. [x] Integrate runtime into the HQ Android application with internal setup and Steam launch APIs.
3. [x] Reuse HQ mod/version compatibility logic for the internal game directory and enable BepInEx under Proton.
4. [x] Replace companion-app UI with internal installation, Steam, mod setup and launch controls.
5. [ ] Build, test and install; verify the internal process/package, mod files/logs and remaining human Steam-login steps.

Step 5 status: APK installed; HQ UI, own Linux r9 installation and internal Steam
login screen verified on the phone. User cannot sign in now, so owned-game
installation, HQ mod files/BepInEx runtime log, voice and gameplay remain pending
that human login step. No existing companion data was changed.

Implementation choices: pinned upstream source with its GPL notices, own app-private runtime, no import of Steam credentials or saves, preserve desktop behavior. Do not install Windows system components (Docker/WSL) or change device-wide developer settings to build this.

## Previous milestone goal
Run HQ Launcher's own UI on the connected Android phone and hand local Lethal Company execution to DroidDeck's supported game-launch interface.

## Design and scope
- Keep the existing desktop frontend/backend and APIs intact.
- Build an Android Tauri shell with a touch-friendly HQ launch screen, runtime detection, setup and direct Lethal Company launch.
- Use DroidDeck's exported `com.droiddeck.launcher.LAUNCH_GAME` intent (`app_id=1966720`, `game_source=STEAM`). Runtime setup and Steam authentication still require DroidDeck/Steam. This first Android slice does not claim desktop HQ mods/version downloads work on Android.
- Prefer the installed DroidDeck runtime over vendoring its native stack. No unverified APK sources, no game files or credentials bundled.

## Steps
1. [x] Inspect project, upstream interface and connected device.
2. [x] Add Android build boundary, native runtime bridge and focused tests.
3. [x] Add HQ Android screen, verify browser behavior and build APK.
4. [x] Install and inspect the app on the phone; report runtime/game limits.

## Result
Android-first milestone delivered: latest ARM64 debug APK installed and HQ UI open on SM-S948N. Native status finds DroidDeck 0.5.0; game handoff to Steam app ID 1966720 was observed in runtime logs. Gameplay/performance and Android HQ mod/version management remain outside this first slice and are not claimed complete. Deferred dual-install selection issue is documented in docs/ANDROID.md.

## Verification
- Real-Cargo Node fixture tests for native-library target selection; browser tests for runtime state and UI behavior.
- Existing Node checks and frontend build.
- Desktop Rust tests/check and ARM64 Android build.
- Browser checks at phone landscape/portrait widths.
- ADB install, launch and app-scoped logcat inspection.

## Review focus
Android must not compile desktop-only OS/TLS/dialog/shortcut dependencies. Native intents must use fixed package/action/app ID; absence must produce an actionable error. The launch screen must distinguish handing off an intent from actual game execution. Desktop entry point and capabilities must remain compatible.

Current verification boundary: Actual modded v81 and managed v73 local-only LAN new games, joystick movement and camera drag verified; existing save was not selected. v73 tap scan and hidden Tab confirmed. English/native version catalog checked; full142 Rust/15 preparation/JVM/wrapper8+1skip/browser tests passed. Integrated managed-v73 installation/selection/mods/wrapper launch passed after waiting for actual Steam subscription readiness. Return to81 selection/launch also reached Launch Mode. Final APK installed, prior installer metadata cleared on new game intent. Diagnostic pluginfolder/config backups fully restored/removed; two incompatible modules intentionally disabled. Voice/multiplayer/sustained sessions remain untested. Leaving HQ on selected81 with both versions installed.

