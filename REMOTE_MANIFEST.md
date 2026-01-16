# Remote Manifest (`manifest.json`)

This project fetches a **remote manifest** from:

- `https://f.asta.rs/hq-launcher/manifest.json`

It is used for:

- **Game download targets** (Steam depot manifest id per game version)
- **Mod list** (Thunderstore packages to install)
- **Per-game-version mod pinning** (optional)
- **Config-chain behavior** for config editor UI (`chainConfig`)

The Rust schema lives in `src-tauri/src/mod_config.rs` (`RemoteManifest`, `ModEntry`).

---

## Top-level schema

```json
{
  "version": 5,
  "manifests": {
    "40": "8596342981027780916",
    "49": "7525563530173177311"
  },
  "chainConfig": [
    ["BepInEx/config/A.cfg", "BepInEx/config/B.cfg"]
  ],
  "mods": [
    {
      "dev": "SomeAuthor",
      "name": "SomeMod",
      "enabled": true,
      "low_cap": 56,
      "high_cap": 73,
      "version_config": {
        "56": "1.2.3",
        "73": "0.0.0"
      }
    }
  ]
}
```

### `version` (number)

An arbitrary **manifest revision number**. It is exposed to the frontend (UI label) but **updates are not driven by this value** anymore.

### `manifests` (map: `"gameVersion"` → `"steamDepotManifestId"`)

Example:

```json
{
  "manifests": {
    "73": "1749099131234587692"
  }
}
```

- **Key**: game version displayed in the launcher (e.g. `73`)
- **Value**: Steam **depot manifest id** string used when downloading that game version

How it’s used:

- The version selector is populated from `Object.keys(manifests)`
- When downloading a game version, the launcher passes this id to DepotDownloader (`-manifest <id>`)

### `chainConfig` (array of arrays of strings)

`chainConfig` is a list of “linked config files”. Each inner array is a group of paths that should be treated as a chain.

Used by the config editor UI: when editing one config file that belongs to a chain group, changes may be applied to the other path(s) in the same group.

### `mods` (array of `ModEntry`)

List of Thunderstore mods that should be installed into:

- `versions/v{gameVersion}/BepInEx/plugins`

The launcher resolves the package + version via Thunderstore’s package list endpoint, but it downloads zips via the direct download URL described below.

---

## `ModEntry` schema

```json
{
  "dev": "AuthorOrNamespace",
  "name": "PackageName",
  "enabled": true,
  "low_cap": 56,
  "high_cap": 73,
  "version_config": {
    "56": "1.2.3",
    "73": "0.0.0"
  }
}
```

### `dev` / `name`

Thunderstore package identity.

- `dev`: package owner/namespace (e.g. `BepInEx`)
- `name`: package name (e.g. `BepInExPack`)

### `enabled` (boolean, default: `true`)

If `false`, the mod is ignored by installer/update-check/update.

### `low_cap` / `high_cap` (optional integers)

Inclusive compatibility bounds for a mod:

- If `low_cap` exists, the mod is skipped when `gameVersion < low_cap`
- If `high_cap` exists, the mod is skipped when `gameVersion > high_cap`

### `version_config` (map: `"gameVersionLowerBound"` → `"version_number"`)

This controls **version pinning** using “threshold pinning”:

- pick the **largest key** \(lower bound\) such that `key <= gameVersion`
- the mapped value is the Thunderstore `version_number` to install

Example:

```json
{
  "version_config": {
    "56": "1.0.1",
    "73": "1.1.1"
  }
}
```

Meaning:

- game `56..72` installs `1.0.1`
- game `73+` installs `1.1.1`

#### Special value: `"0.0.0"`

If the pinned `version_number` is `"0.0.0"`, it is treated as **“no pin”**.

That means:

- the launcher falls back to **latest version** instead of a pinned one

This is implemented in `ModEntry::pinned_version_for()` in `src-tauri/src/mod_config.rs`.

---

## How “latest mod version” is resolved

The launcher fetches the package list:

- `https://thunderstore.io/c/lethal-company/api/v1/package/`

It then selects the latest version by:

- parsing `version_number` as semver (loose parsing)
- choosing the **maximum semver**

For downloading the zip, the launcher **constructs** the URL:

- `https://thunderstore.io/package/download/{dev}/{modname}/{version}/`

---

## Notes / gotchas

- All keys inside `manifests` / `version_config` are strings in JSON, but are parsed as `u32` in Rust.
- The launcher expects the root key to be `chainConfig` (camelCase) when serving data to the frontend, but the remote manifest currently uses `chain_config` in Rust structs only as a field name; JSON uses whatever is provided. Keep it consistent with what your hosted JSON actually emits.
  - Current Rust `RemoteManifest` expects `chain_config` as the JSON key (snake_case).
  - Frontend receives `chain_config` via the `get_manifest` command.

