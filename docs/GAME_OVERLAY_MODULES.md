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
  Setting.color("color", "Color", "#ffffff"),
  Setting.selectMenu("align", "Align", [
    { label: "Left", value: "left" },
    { label: "Right", value: "right" }
  ], "left")
]);

register("visible", ({ settings, context }) => settings.enabled !== false || context.editMode);

register("renderOverlay", ({ context, settings, api }) => {
  const quota = api.valueAt(context.lcstats, "QuotaInfo.NewQuota", 0);
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

## Settings Inputs

- `Setting.toggle(key, label, defaultValue)`: true/false checkbox.
- `Setting.color(key, label, defaultValue)`: color picker.
- `Setting.range(key, label, min, max, step, defaultValue)`: slider.
- `Setting.number(key, label, defaultValue, min, max, step)`: numeric input.
- `Setting.key(key, label, defaultValue)`: key capture button. Values are strings like `Insert`, `Ctrl+Shift+K`, or `Ctrl+Shift+*`.
- `Setting.hotkey(key, label, defaultValue)`: alias for `key`.
- `Setting.text(key, label, defaultValue)`: single-line text input.
- `Setting.textarea(key, label, defaultValue)`: multi-line text input.
- `Setting.select(key, label, options, defaultValue)`: select menu.
- `Setting.selectMenu(key, label, options, defaultValue)`: alias for `select`.

`context` includes `elapsedSeconds`, `lcstats`, `lcstatsRaw`, `lcstatsAgeMs`, `displayTimeMs`, `leaderboard`, `endSummary`, `editMode`, and formatting helpers. `context.recordChecker` is kept as a deprecated alias for `context.leaderboard`. Use `api.html(value)` for untrusted text before returning HTML.

## Leaderboard Data

`context.leaderboard` contains the current HighQuotaHQ lookup state plus metadata copied from the `highquotahq` app:

- `status`: `"idle" | "waiting" | "loading" | "ready" | "error"`.
- `collections.vanilla`: `leaderboards_hq`, `leaderboards_sdc`, `leaderboards_smhq`.
- `collections.modded`: `modded_hq`, `modded_sdc`, `modded_smhq`.
- `collections.legacyModded`: `lc_modded_brutal_*`, `lc_modded_eclipsed_*`, `lc_modded_wesleysmoons_*`, `lc_modded_classicmoons_*`.
- `boardTypes`: `hq`, `sdc`, `smhq` with `name`, `metricLabel`, and `metricKey`.
- `runFields`: known run fields such as `players`, `version`, `verified`, `quotaAmount`, `quotaReached`, `totalScrap`, `moon`, `scrapType`, `videos`, `date`, `verifiedAt`, and `verifier`.

When ready, common values include `score`, `rank`, `totalRecords`, `top`, `next`, `nextScore`, `track`, `collectionName`, `boardType`, `version`, and `moon`.
