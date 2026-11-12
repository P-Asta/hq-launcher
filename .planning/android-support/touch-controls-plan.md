# Lethal touch controls implementation plan

Goal: Replace the generic gamepad overlay during HQ game sessions with the exact requested joystick/actions and Amethyst-style touch routing. Keep internal setup, Steam authentication and desktop APIs intact.

Architecture: A small pure Java input state machine owns pointer identities, held keys/buttons and gesture timing; an Android Canvas View owns layout and feeds the existing compositor. A pinned SessionActivity patch wires the view and lifecycle. Rust's existing verified Steam depot/version lookup caches native control metadata, keyed by depot; launch passes the current depot so stale or unknown versions never expose Tab.

Files: android/runtime/src/main/java/com/droiddeck/launcher/hq/GameTouchInput.java; GameTouchControls.kt; src/test/java/.../GameTouchInputTest.java; patches/hq-touch-controls.json; runtime.lock.json; HqRuntimeBridge.kt; HqRuntimePlugin.kt; src-tauri/src/android.rs,android_mods.rs; docs/ANDROID.md.

Constraints: no new dependency, no external app/data access, no saved generic-controller preference changes. E18/G34/Tab15 are Linux evdev keys; joystick W17/A30/S31/D32. Tab requires known game version>=80. Retain keyboard/HQ menu/Esc access. Amethyst behavior: relative drag while pointer locked, short stationary tap<=150ms right click, stationary300ms left press until release; direct menu tap-left and drag-left. No click on cancellation or grab transition. Multiple simultaneous joystick/button/look pointers must not steal or jump. Release all held inputs on pause/cancel/hidden/modal/rotation.

- [x] RED: pure-JVM tests for diagonal movement/release, joystick+E+camera, held E/G, canceled right taps, drag suppression,300ms hold/release, pointer replacement without jump, grab transition, scroll signs and80-only Tab.
- [x] GREEN: minimal input state and neutral translucent joystick/action View; per-pointer hit regions and dp/cutout layout; callbacks use existing guest-key/pointer/button/wheel APIs.
- [x] Version boundary: native cache keyed by trusted depot mapping; current depot from existing offline preflight; version floor enforced in input/layout; unknown hidden.
- [x] Wire only HQ game sessions; preserve install session controls and release on existing modal/lifecycle state changes.
- [x] Focused review, actual JVM checks, preparation checks, Android build, data-preserving APK update and real-device layout/input checks. Joystick/camera/tap scan verified in actual modded ship; not every mod or multiplayer feature is claimed.

Startup stall isolated to PathfindingLib 2.4.1; optional OffMesh hook disable was insufficient. Android disables it and dependent PathfindingLagFix with a visible reason, preserving the other mods and desktop policy.

Device: modded v81 and managed v73 both reached a new local-only LAN ship. Joystick and camera responded; short tap scan also verified on v73. Tab shown on81 and hidden on73. Final Keys label fits; fresh game intent clears prior installer metadata so controls remain visible after version setup.
