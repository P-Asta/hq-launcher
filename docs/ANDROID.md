# Android

HQ Launcher embeds its own Linux/Steam runtime and runs Lethal Company locally
on the phone. The runtime, Steam installation, game files and HQ mod state belong
to the `asta.hq_launcher` app. A separately installed DroidDeck app and a connected
PC are not required to play. Downloads, Steam sign-in and online Steam features
still need an internet connection; this does not guarantee Steam offline mode.

The native runtime is integrated from pinned DroidDeck 0.5.0 source and matching
ARM64 components. HQ owns the launcher UI and application; Steam and the game use
an internal native session screen. It does not import another app's Steam
credentials, game saves or private data.

## Phone setup

1. Open HQ Launcher and select **Linux 실행 환경 설치**. Keep enough free space for
   the Linux runtime, Steam, the game and mods. The launcher displays installation
   progress and any download or storage error.
2. Select **리썰 컴퍼니 설치**. Sign in through the internal normal Steam window and
   install your legitimately owned copy of Lethal Company. Steam opens the fixed
   installation page for app ID `1966720`. HQ requests desktop UI for installation
   and game launch, avoiding Steam Deck/Big Picture navigation. Installation uses
   compact keyboard/menu controls; normal gameplay keeps its touch controls.
   HQ does not provide game files or a
   Steam account.
3. Return to HQ Launcher. Before changing mods, select **실행 환경 종료** and wait
   for the session to stop. Select **HQ 모드 적용** when the installed game version
   is supported. After the first successful application, individual mod switches
   become available. Both application and toggles require the session to remain
   stopped.
4. Select **게임 실행**. The request opens the internal session; accepting that
   request alone does not prove that the game has started. Confirm the result in
   the session screen. A supported HQ mod set is optional for base-game launch.

If Android reports a child-process restriction, HQ shows **Android 개발자 설정
열기**. Review the device setting and return to the launcher. HQ opens the OS
settings screen; it does not silently change system-wide developer settings.

Game voice chat is off by default. Before starting a session, enable **게임
마이크 사용** and approve Android's microphone permission prompt yourself. If
permission is denied, the switch stays off and HQ shows an error; retry when
ready to grant permission. Microphone settings require an idle runtime with no
installation or mod work in progress and take effect from the next session.

Game compatibility is determined from the installed Steam depot manifest and
HQ's supported-version catalog. An unknown version, incomplete Steam update or
failed compatibility lookup does not count as successful mod support. Finish
Steam installation or updating, stop the session, then refresh and retry. The
mod installer uses the existing HQ package rules, installs BepInEx and configures
the game's Proton `winhttp` override. Installing or updating mods and fetching
the compatibility catalog require internet access. The desktop version picker,
practice presets, overlay and config editor are not part of this Android screen.

## Prepare the embedded runtime

The build needs Node/Yarn, Python 3, Git, Rust with the `aarch64-linux-android`
target, JDK 17, and the Android SDK/NDK with SDK platform 36. Accept the SDK
licenses before building. The tracked Android Studio project is
`src-tauri/gen/android`; do not rerun `android init` over its native integration.

Follow [the runtime library instructions](../android/runtime/README.md) for the
pinned source and local APK inputs. Place the exact matching donor APK at
`.android-runtime.local/droiddeck-donor.apk`, or pass `--apk PATH` to the
preparation script. This APK is a build input for matching assets/native files;
installing its app on the phone is not required. An arbitrary APK or third-party
mirror is not a substitute for the locked donor.

```powershell
yarn install --immutable
python scripts/prepare-android-runtime.py
python scripts/test-android-runtime-preparation.py
python scripts/prepare-android-runtime.py --verify-only
```

Preparation clones the pinned source when absent and rejects an existing source
checkout with the wrong commit or tracked modifications. It verifies the donor
hash, size, archive contents and ARM64 ELF inputs, then prepares the ignored
`.android-runtime.local/prepared` directory. Donor DEX, API keys, signatures,
accounts and save data are not imported. Gradle verifies the prepared inventory
and source before compiling the embedded `:hqRuntime` library. This matching
source/donor workflow does not need Docker, WSL or a native-runtime rebuild.

The app compiles against SDK 36 but currently targets SDK 28, with minimum SDK 28
and ARM64 only. The embedded runtime executes a Linux userland from the app's
private storage and depends on the execution policy used by pre-29 target apps.
Raising `targetSdk` needs a separate runtime execution-policy migration. A recent
compile SDK does not remove that requirement. Some supplied native components
have 4 KiB ELF load alignment; support on 16 KiB page-size devices still needs
validation and may require rebuilt components.

## Build and deploy

```powershell
yarn android:build:debug
adb install -r src-tauri/gen/android/app/build/outputs/apk/arm64/debug/app-arm64-debug.apk
adb shell am start -n asta.hq_launcher/.MainActivity
```

`yarn android:build` builds an ARM64 release APK; configure release signing before
distribution. Both APK build commands run runtime preparation before invoking
Tauri. The Windows helper falls back to copying the compiled native library when
symlink permissions are unavailable; it does not enable Windows Developer Mode.
`yarn android:dev` uses Tauri's live development workflow and can require that
symlink permission. USB/ADB is useful for development and initial deployment,
but it is not the game's execution host or a play-time dependency.

The combined build contains GPL-3.0 source and third-party runtime components.
Preserve their notices and corresponding source obligations when distributing.
Runtime license assets are packaged under `assets/licenses/droiddeck`; see the
[runtime library README](../android/runtime/README.md) and
[lock file](../android/runtime/runtime.lock.json) for provenance and pins.

## Interface and verification

Android shares the desktop launcher's workspace, toolbar, search and mod rows.
Its launcher and HQ touch controls use English, matching the desktop interface.
Startup, preparation, installation and busy indicators share the HQ logo reveal.
The internal runtime uses the same SVG for loading, pause and return/stop
transitions; actual progress and cancel/retry/log controls are preserved.
Reduced motion keeps the mark static without replacing known progress with a
full logo. Runtime loading fades away on its existing first-frame signal.
Below 800 CSS pixels, tools wrap, search takes its own row, and panels stack;
desktop panel resizing and fixed minimum widths apply only above that boundary.
Layout follows width independently of platform. Android uses its own fixed
runtime commands while desktop keeps its existing commands and version APIs.

The version menu uses HQ's manifest catalog. Selecting an uninstalled version
downloads its fixed app/depot manifest through the internal authenticated Steam
client. HQ copies completed downloads to separate version directories and keeps
their own Proton prefix and mod state; Steam's current game files and manifest
are retained. A fixed `%command%` wrapper preserves Steam's app identity and
runtime arguments while selecting the game folder. A version is selectable only
after its matching installation receipt and required files are present.
On the connected phone, v73 was downloaded, selected, given its matching HQ mods
and launched into a local-only LAN ship. Movement and camera drag worked, and
the v80+ Tab control stayed hidden. Steam's primary manifest remained v81.
Selecting v81 again also reached its launch-mode screen through the wrapper.
Steam Cloud still follows the same app identity; a separate prefix does not
disable or isolate Steam Cloud saves.

In the tested runtime, adding PathfindingLib 2.4.1 alone reproduced the black
startup; disabling its optional OffMesh hook did not avoid it. Android game v73+
therefore disables PathfindingLib and its dependent PathfindingLagFix, with a
visible reason. Other HQ mods and the desktop policy are retained. This records
the observed runtime incompatibility; the precise native detour failure has not
been established.

Game sessions use HQ's native joystick, E/G/scroll, Ctrl (hold), Shift (toggle),
right-click (hold) and Space controls. Tab is available
only when the verified Steam depot maps to game v80 or later; unknown versions
hide it. Camera drag is relative while the game locks the pointer. A stationary
tap within 150ms sends left click; holding 300ms holds left click until release.
Menus use direct left taps and drags. Cancel, focus loss and modal overlays
release held inputs, including toggled Shift. New touch gestures retain toggled
Shift. HQ opens only Mouse sensitivity (0.25–3×) and Key layout. The native layout
editor supports dragging buttons/joystick and resizing individual controls
(75–175%, with a 44dp button minimum), Reset and explicit Done to save. Settings
are stored in HQ private preferences and clamped to the current safe viewport.
Editing never sends guest input. This is an original implementation of the input behavior
inspected in [Amethyst Android](https://github.com/AngelAuraMC/Amethyst-Android)
at commit `330c6eae3164df64bdc4828e946a9e62cc5169e4`, without Minecraft dependencies.
Cold starts choose the HQ controls from the fixed game launch request before the
service publishes its active session URL. Installation sessions retain keyboard
and setup controls.

The frontend invokes fixed internal commands for runtime status/installation,
Steam installation, session stop, game launch and HQ mod management. It does not
accept an arbitrary app package, Steam URI or executable path. Game paths are
resolved inside HQ's private Linux root (`files/linuxfs`), and native mod
writes require a stopped session and a completed compatible Steam installation.

For browser checks, select the Android frontend before starting Vite:

```powershell
$env:TAURI_ENV_PLATFORM = 'android'
yarn dev
```

Run `scripts/test-android-ui.cjs` using Playwright MCP's
`browser_run_code_unsafe` filename option against `http://127.0.0.1:1421`. It
mocks the native IPC/event boundary and exercises runtime prerequisites,
installation progress, internal Steam actions, mod application/toggles, session
locks, errors/retry, foreground refresh and responsive touch controls. Browser
fixtures do not execute Linux, Steam, Proton or the game.

`scripts/test-responsive-launcher.cjs` also mounts the actual desktop
`LauncherPage` with an IPC fixture and checks selected-mod panels, resize-handle
transitions, usable search width and overflow across 320–1280px.

The internal ARM64 debug APK was installed and tested on SM-S948N, Android 16
(4096-byte pages). HQ's 384×779 launcher viewport, native IPC, missing-game
guards, fresh Linux r9 installation, and the internal Steam login screen were
verified. The session activity, proot and Steam processes belong to
`asta.hq_launcher`; native logs use its own private files directory. The real
session also reported `stopping` as busy until teardown finished.

Checks passed: 142 Rust tests, 15 runtime-preparation tests, JVM runtime/input rules,
Playwright UI checks including microphone denial/retry, existing Node checks,
desktop/Android frontend builds, and ARM64 APK packaging. Source review fixes
cover stale mods after game updates, launch/write exclusion and the Android
DocumentsProvider startup contract.

After isolating the startup stall described above, the updated APK loaded
BepInEx 5.4.23.4 and 16 plugins on game v81. A local-only LAN new game reached
the ship; joystick movement and camera drag were verified on the phone. The
existing save was not selected. All diagnostic plugin/config moves were
restored; only the two explicitly blocked modules remain disabled. Some Harmony
patch warnings and a LightsOut instantiation warning remain, so every individual
mod feature is not established. Microphone audio, multiplayer and sustained
gameplay remain unverified.

The game-specific installer uses normal Steam UI. The compositor still uses
Steam app IDs for selecting the Lethal Company window; this does not turn on
Big Picture or the Steam Deck client flags. Installation uses compact menu and
keyboard controls, including when resumed; gameplay keeps the game controls.
Steam updates to Proton must finish before playing: an interrupted Proton
update caused a missing `atl.dll` on the device, and Steam's completed update
restored it without manually changing Wine libraries or the game prefix.
Android displayed a debug compatibility notice for donor libraries lacking
16 KiB alignment; this device uses 4 KiB pages. The notice does not establish
support for 16 KiB devices, which may require rebuilt native components.
