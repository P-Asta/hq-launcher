# HQ touch control settings

User confirmed key layout means on-screen position and size, not key rebinding. Keep HQ UI English. HQ button opens only Mouse sensitivity and Key layout (+close). Preserve installer controls, desktop, native runtime, mods/versions/saves and logo transitions.

1. [x] Inspect existing pure input model, canvas overlay, lifecycle and SessionActivity patch.
2. [x] RED missing actions/APIs; GREEN Ctrl29/Space57, ref-counted RMB273, latched Shift42, left surface taps, sensitivity multiplier. Full cancel/grab transition releases latch; new gesture cancels touches only.
3. [x] Persist sensitivity(.25–3x) and normalized positions/per-control sizes; modal layout editor with drag, resize, reset and explicit Done. Dialog focus releases all game inputs; editing never sends guest input.
4. [x] Add/reflow Ctrl/Shift/Right click/Space in existing canvas; HQ hit priority guarantees settings remain reachable after overlapping custom layouts. Keep E/G/scroll and verified80+ Tab floor.
5. [x] Focused geometry/input tests, preparation/build/review; install update preserving phone data. Actual settings/edit/save/reopen, new game controls and lifecycle verification. Preserve currently selected version.

Files: GameTouchInput.java/Test.java; GameTouchControls.kt; new TouchControlGeometry.java/Test.java, GameTouchSettings.kt; existing hq-touch-controls.json; docs/ANDROID.md. No dependency additions.

Safety: all latched/held input released on focus loss, hidden controls, keyboard/settings, pointer unlock, detach or version changes. Ordinary ACTION_DOWN only clears finger state to preserve sprint toggle. Unknown/old versions hide utility during play. Saved values validated/clamped and adapted to viewport/cutout; reset restores defaults.

Verification so far: input+geometry JVM GREEN, preparation15/15, final Kotlin/APK Gradle exit0, fresh reviewer no remaining actionable findings, adb update Success. Actual device settings/edit/save/reopen/reset, modded73 gameplay and cold restart verified. Selected73 retained.

Cold-start finding: native Activity initializes controls before Service fills volatile steamUrl; initial SessionEvents.begin clears it and moves phase to PREPARING, so subsequent idle-only metadata copy is skipped. Choice stays generic until onResume refresh. Confirmed API resume with populated activeURL restores HQ immediately. Use explicit fixed-game launch URL as fallback only before activeURL exists, preserving installer veto/active-session priority. Regression RED observed cold assertion, fix GREEN/build/device regression passed. Cold prefs1.80/E125% already verified; Reset restored1.00/empty layout.
