# Game Overlay Modules

HQ Launcher loads editable game overlay widgets from the app data `overlayModule` directory. Each `.js` file is evaluated with a small ChatTriggers-like API.

The launcher also writes `hq-overlay-module.d.ts` into that directory. For VS Code autocomplete, add this at the top of a module:

```js
/// <reference path="./hq-overlay-module.d.ts" />
// @ts-check
```

## Minimal Module

```js
/// <reference path="./hq-overlay-module.d.ts" />
// @ts-check

setName("Quota Alert");
setDescription("Shows the latest quota value from LCStatsTracker.");
setDefaultPosition({ x: 8, y: 10 });
setWrapperClass("rounded border border-white/15 bg-black/70 p-3");

register("settings", [
  Setting.toggle("enabled", "Enabled", true),
  Setting.key("toggleKey", "Toggle Key", ""),
  Setting.color("color", "Color", "#ffffff"),
  Setting.selectMenu("align", "Align", [
    { label: "Left", value: "left" },
    { label: "Right", value: "right" }
  ], "left")
]);

let runtimeEnabled = null;
register("tick", ({ settings, api }) => {
  if (runtimeEnabled == null) runtimeEnabled = settings.enabled !== false;
  if (settings.toggleKey && api.input.consumePress(settings.toggleKey)) {
    runtimeEnabled = !runtimeEnabled;
  }
});

register("visible", ({ context }) => runtimeEnabled || context.editMode);

register("renderOverlay", ({ settings, api }) => {
  const quota = api.valueAt(api.getLcStats(), "QuotaInfo.NewQuota", 0);
  return `<div class="overlay-title" style="color:${settings.color}">Quota</div>
    <div class="overlay-value">${api.number(quota)}</div>`;
});
```

## API

- `setName(name)`: Display name in the overlay settings panel.
- `setDescription(description)`: Help text shown for the selected module.
- `setLocked(true)`: Prevent layout dragging and snap settings.
- `setDefaultPosition({ x, y })`: Default top-left percentage position.
- `setDefaultSettings(settings)`: Default setting values.
- `setWrapperClass(className)`: Tailwind classes applied to the module wrapper.
- `setCss(css)`: CSS injected while the module is loaded.
- `register("settings", schema)`: Settings shown in the UI.
- `register("visible", fn)`: Return `false` to hide the module.
- `register("derive", fn)`: Compute data passed into render handlers.
- `register("renderOverlay", fn)`: Return an HTML string.
- `register("tick", fn)`: Run before visibility/render checks. Useful for input-driven module state.

## Settings Inputs

- `Setting.toggle(key, label, defaultValue)`: true/false checkbox.
- `Setting.color(key, label, defaultValue)`: color picker.
- `Setting.range(key, label, min, max, step, defaultValue)`: slider.
- `Setting.number(key, label, defaultValue, min, max, step)`: numeric input.
- `Setting.key(key, label, defaultValue)`: key capture button. Values are strings like `Insert`, `Ctrl+Shift+K`, or `Ctrl+Shift+*`.
- `Setting.hotkey(key, label, defaultValue)`: alias for `key`.
- `Setting.image(key, label, defaultValue)`: image upload. Values are `data:image/...` URLs that can be used as an `<img src>`.
- `Setting.text(key, label, defaultValue)`: single-line text input.
- `Setting.textarea(key, label, defaultValue)`: multi-line text input.
- `Setting.select(key, label, options, defaultValue)`: select menu.
- `Setting.selectMenu(key, label, options, defaultValue)`: alias for `select`.

`context` includes `elapsedSeconds`, `lcstats`, `lcstatsRaw`, `lcstatsAgeMs`, `streamOverlays`, `streamOverlaysAgeMs`, `displayTimeMs`, `leaderboard`, `endSummary`, `editMode`, and formatting helpers. `context.recordChecker` is kept as a deprecated alias for `context.leaderboard`. Use `api.html(value)` for untrusted text before returning HTML.

## Short API Helpers

Most modules can stay compact by reading live data through `api`:

```js
register("renderOverlay", ({ api }) => {
  const stats = api.getLcStats();
  const stream = api.getStreamOverlay();
  const moon = stream?.moonName ?? api.valueAt(stats, "Moon", "Unknown");
  const quota = stream?.quotaValue ?? api.valueAt(stats, "QuotaInfo.NewQuota", 0);

  return `<div>${api.html(moon)}</div><div>${api.number(quota)}</div>`;
});
```

- `api.getLcStats()`: latest LCStatsTracker payload view.
- `api.getLcStatsRaw()`: raw latest LCStatsTracker payload string, or `null`.
- `api.getStreamOverlay()`: latest StreamOverlays payload, or `null`.
- `api.valueAt(root, path, fallback)`: read a nested value from an object.
- `api.valueAtAny(root, paths, fallback)`: read the first matching nested value.
- `api.number(value)`: numeric display formatting.
- `api.html(value)`: HTML escaping.

StreamOverlays reading is opt-in. Enable `Use StreamOverlays API` in the launcher Overlay Settings when a module needs it.

## Input API

Use `Setting.key(...)` for a configurable shortcut and `api.input` inside `tick`, `visible`, `derive`, or `renderOverlay`.

```js
let enabled = true;

register("settings", [
  Setting.key("toggleKey", "Toggle Key", "Ctrl+K")
]);

register("tick", ({ settings, api }) => {
  if (api.input.consumePress(settings.toggleKey)) {
    enabled = !enabled;
  }
});

register("visible", () => enabled);
```

- `api.input.consumePress(shortcut)`: one-shot key down check, best for toggles/actions.
- `api.input.consumeRelease(shortcut)`: one-shot key release check.
- `api.input.down(shortcut)` / `held(shortcut)` / `shortcut(shortcut)`: true while the shortcut is held.
- `api.input.pressed(shortcut)` / `released(shortcut)`: transient recent press/release checks.
- `api.input.events()`: recent normalized input events.

Shortcuts use the same strings as key settings, such as `Insert`, `K`, `Ctrl+Shift+K`, or `Numpad1`.

## Leaderboard Data

`context.leaderboard` contains the current HighQuotaHQ lookup state plus metadata copied from the `highquotahq` app:

- `status`: `"idle" | "waiting" | "loading" | "ready" | "error"`.
- `collections.vanilla`: `leaderboards_hq`, `leaderboards_sdc`, `leaderboards_smhq`.
- `collections.modded`: `modded_hq`, `modded_sdc`, `modded_smhq`.
- `collections.legacyModded`: `lc_modded_brutal_*`, `lc_modded_eclipsed_*`, `lc_modded_wesleysmoons_*`, `lc_modded_classicmoons_*`.
- `boardTypes`: `hq`, `sdc`, `smhq` with `name`, `metricLabel`, and `metricKey`.
- `runFields`: known run fields such as `players`, `version`, `verified`, `quotaAmount`, `quotaReached`, `totalScrap`, `moon`, `scrapType`, `videos`, `date`, `verifiedAt`, and `verifier`.

When ready, common values include `score`, `rank`, `totalRecords`, `top`, `next`, `nextScore`, `track`, `collectionName`, `boardType`, `version`, and `moon`.
