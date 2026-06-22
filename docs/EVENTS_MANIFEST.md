# Events Manifest (`events.json`)

The launcher fetches an optional `events.json` from the same remote directory as
`manifest.json`.

- Stable: `https://f.asta.rs/hq-launcher/events.json`
- Beta: `https://f.asta.rs/hq-launcher/beta/events.json`

If the file is missing, the launcher treats it as an empty event list.

## Minimal Example

```json
{
  "version": 1,
  "events": [
    {
      "id": "summer",
      "name": "Summer Event",
      "versions": [73],
      "testers": ["76561198000000000"],
      "preset": "hq",
      "starts_at": "2026-06-22T00:00:00Z",
      "ends_at": "2026-07-15T23:59:59Z",
      "image": "https://f.asta.rs/hq-launcher/events/summer.webp",
      "links": {
        "discord": "https://discord.gg/example",
        "website": "https://example.com"
      },
      "mods": [
        {
          "dev": "Author",
          "name": "ModName"
        },
        {
          "dev": "Author2",
          "name": "ModName2",
          "version_config": {
            "73": "1.2.3"
          }
        }
      ]
    }
  ]
}
```

## Fields

### `version`

Events manifest revision. It is informational and does not drive launcher
updates.

### `events`

Array of event entries.

### `id`

Required stable event id. Use lowercase URL-safe names such as `summer` or
`weekly-01`. If an event id disappears from `events.json`, the launcher clears
the selected event and disables the event mods it previously enabled.

### `name`

Required display name shown in the launcher.

### `versions`

Optional array of allowed game versions. If omitted or empty, every version from
`manifest.json` is allowed.

### `testers`

Optional tester allowlist for private testing. Prefer SteamID64 values.

```json
"testers": ["76561198000000000", "yoostar33"]
```

If omitted or empty, the event is public. If present, the launcher first matches
SteamID64 values. Non-SteamID64 entries are matched against the logged-in Steam
username as a fallback. The backend also rejects preparing tester-only events for
non-testers. Testers can see tester-only events before `starts_at`; non-testers
only see public events after `starts_at`.

### `preset`

Optional default preset/run mode used by the event. Defaults to `hq`.

Use values that match existing launcher presets, such as:

- `hq`
- `smhq`
- `brutal`
- `brutal_smhq`
- `brutal_eclipsed`
- `wesley`
- `wesley_smhq`
- `wesley_eclipsed`
- `c_moons`
- `c_moons_smhq`
- `c_moons_eclipsed`
- `eclipsed_hq`

### `starts_at`

Optional event start time in UTC RFC3339 format. Use `Z` for UTC.

```json
"starts_at": "2026-06-22T00:00:00Z"
```

Before this time, the frontend hides the event from non-testers. Tester-only
events remain visible to matching testers before this time. If omitted, the
event is active immediately.

### `ends_at`

Optional event end time in UTC RFC3339 format. Use `Z` for UTC.

```json
"ends_at": "2026-07-15T23:59:59Z"
```

When the time has passed, the frontend hides the event. If the hidden event was
selected, the launcher clears it and disables the event mods it previously
enabled.

### `image`

Optional image URL for the event card.

### `links`

Optional external links. Missing links are hidden in the UI.

```json
"links": {
  "discord": "https://discord.gg/example",
  "website": "https://example.com"
}
```

### `mods`

Optional event-only Thunderstore mods. These use the same shape as
`manifest.json` mod entries, but normally only `dev` and `name` are needed.

Supported fields include:

- `dev`
- `name`
- `enabled`
- `low_cap`
- `high_cap`
- `version_config`

Event mods are installed after the base preset is prepared. When switching from
one event to another, the previous event's remembered mods are disabled and any
mods that were newly installed by the previous event are removed. If the event
expires, is removed, or events are disabled in Settings, newly installed event
mods are removed while pre-existing user-installed mods are only disabled.
