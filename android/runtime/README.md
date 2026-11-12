# HQ Android runtime library

HQ owns the application, launcher activity, UI, and private runtime data. This
library embeds DroidDeck 0.5.0 source and compatible native runtime components;
installing a separate DroidDeck application is not required.

`runtime.lock.json` pins both the source commit and a local donor APK. The donor
was pulled from the authorized phone installation. Its DEX `BuildConfig` reports
the same source commit. Only APK assets and ARM64 native runtime files are
reused. DEX, API keys, signatures, accounts, save data, and donor APK resources
are not imported.

Prepare the ignored local inputs before building:

```powershell
python scripts/prepare-android-runtime.py
python scripts/test-android-runtime-preparation.py
python scripts/prepare-android-runtime.py --verify-only
```

The script clones the pinned public source if `.android-runtime.local/DroidDeck`
is absent. Existing checkouts must have the exact commit and no tracked edits;
the script never resets an existing checkout. Supply the matching local APK at
`.android-runtime.local/droiddeck-donor.apk`, or pass `--apk PATH`. It does not
download an unpinned donor or read an installed app's private data. A different
donor requires an explicit lock/source compatibility review. No Docker, WSL, or
native rebuilding is needed for this matching source/donor pair.

Preparation validates the APK SHA-256, size, archive paths, regular-file types,
required assets, and ARM64 ELF headers. It writes the HQ integration manifest,
runtime payload, GPL license, third-party notices, source attribution, and an
output hash inventory to `.android-runtime.local/prepared`. Gradle verifies
that inventory and source checkout before compiling. Source Java/Kotlin is
staged with tracked patches that return installation notifications to HQ and
leave other applications' sessions untouched. Source before/after hashes and
patch hashes are recorded without modifying the pinned checkout. Source and
resources compile into `:hqRuntime` alongside HQ's bridge. Runtime activity and
service entry points remain internal. The documents provider preserves Android's
required exported contract, protected by `android.permission.MANAGE_DOCUMENTS`
and URI grants so the system document picker can access it. Other providers
remain internal; the parent app supplies its own Application.

Support JNI listed in `aar_owned_libraries` comes from the exact upstream Maven
dependencies instead of being packaged twice. `libtermux.so` is the single
explicit duplicate exception inherited from upstream's custom PTY native code.
There is no broad native-library pick-first rule.

The Android app uses compile SDK 36, target SDK 28, Kotlin/Compose plugin 2.1.0,
Java 17, and ARM64. The runtime executes a Linux userland from private storage;
raising target SDK requires a separate execution-policy migration. Some donor
libraries have 4 KiB ELF load alignment, so 16 KiB devices require validation
and potentially rebuilt components. This integration does not claim 16 KiB
support solely because HQ uses a recent Android toolchain.

Gradle copies `public/icon.svg` into generated HQ assets for the runtime's logo
transitions. The tracked loading patches preserve readiness, progress and error
controls, use HQ branding for loading/pause/exit, and respect disabled animations.

The upstream source is GPL-3.0. Its GPL license and available third-party
notices are packaged under `assets/licenses/droiddeck`; existing donor license
assets are preserved. Keep the pinned source and integration changes available
when distributing a combined build, in accordance with the applicable licenses.
