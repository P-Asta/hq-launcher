# Steam CEF compatibility helper

Adapted from https://github.com/notpop/steam-on-m1-wine/blob/main/wrapper/src/steamwebhelper-wrapper.c (MIT; see LICENSE).

Wine 11 on Apple Silicon displayed a black Steam login window despite Steam's
CEF GPU-disable switches. Forwarding `--disable-gpu --single-process` directly
to its CEF helper restored the login UI, and a user completed login and started
Lethal Company on an M1. Single-process CEF reduces browser process isolation;
this workaround is confined to the launcher's managed Windows Steam prefix.

The launcher retains Valve's original binary as `steamwebhelper_real.exe`.
It refreshes that backup when Steam updates and atomically reapplies the shim.
Normal Steam update checks remain enabled. `-noverifyfiles` skips bootstrap
verification that would otherwise replace this intentional shim on each start.
A monitor reapplies the shim and restarts only CEF processes after an update.
Custom Wine/CrossOver environments are not patched.

The small PE binary is embedded so users and macOS builders do not need MinGW.
Rebuild it from the repository root with:

```sh
x86_64-w64-mingw32-gcc -municode -O2 -Wall -Wextra -static -mwindows -s -Wl,--no-insert-timestamp src-tauri/resources/macos-steam/steamwebhelper-wrapper.c -o src-tauri/resources/macos-steam/steamwebhelper-wrapper.exe
```

HQ adds an embedded marker to recognize its shim without relying on file size.
The updater must never overwrite the original backup with a shim. Rust tests
cover repeated preparation, updated Valve binaries, and interrupted setup.
