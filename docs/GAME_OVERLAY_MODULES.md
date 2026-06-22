# Game Overlay Modules

HQ Launcher loads editable overlay widgets from the app data `overlayModule` directory. JavaScript files placed directly in that folder become active modules. Files under `overlayModule/example` are examples only; copy one to the parent folder when you want to run it.

The launcher also writes `hq-overlay-module.d.ts` into `overlayModule`. Add this header to a module for VS Code autocomplete:

```js
/// <reference path="./hq-overlay-module.d.ts" />
// @ts-check
```

## Mental Model

- One `.js` file can define one overlay by calling `setName`, `register`, and related helpers at the top level.
- One overlay module has one settings panel.
- One module can render many draggable overlay items by returning an array from `renderOverlay`.
- Each rendered overlay item can get its own saved position while sharing the same module settings.
- Overlay ids must stay stable. HQ Launcher stores settings and positions by id.
- Modules return HTML strings. Escape user or external data with `api.html(value)`.

## Single Overlay

```js
/// <reference path="./hq-overlay-module.d.ts" />
// @ts-check

setName("Quota Alert");
setDescription("Shows the latest quota value from LCStatsTracker.");
setDefaultPosition({ x: 8, y: 10 });
setWrapperClass("rounded border border-white/15 bg-black/70 p-3");

register("settings", [
  Setting.toggle("enabled", "Enabled", true),
  Setting.color("color", "Color", "#ffffff")
]);

register("visible", ({ context, settings }) => context.editMode || settings.enabled !== false);

register("renderOverlay", ({ api, settings }) => {
  const quota = api.valueAt(api.getLcStats(), "QuotaInfo.NewQuota", 0);
  return `<div style="color:${api.html(settings.color)}">Quota ${api.number(quota)}</div>`;
});
```

## Multiple Overlay Items From One Setting

Use `Setting.images(...)` when one config value should accept repeated uploads. Return an array from `renderOverlay`; every item becomes a separate draggable overlay on screen, but the settings panel still shows only one module.

```js
/// <reference path="./hq-overlay-module.d.ts" />
// @ts-check

setName("Multi Image Uploads");
setDefaultPosition({ x: 60, y: 12 });
register("settings", [
  Setting.toggle("enabled", "Enabled", true),
  Setting.images("images", "Images", []),
  Setting.range("width", "Width", 48, 900, 1, 220)
]);

register("visible", ({ context, settings }) => context.editMode || (settings.enabled !== false && (settings.images ?? []).length > 0));

register("renderOverlay", ({ settings, api }) => {
  const images = Array.isArray(settings.images) ? settings.images : [];
  return images.map((src, index) => ({
    id: `image-${index + 1}`,
    defaultPosition: { x: 60 + index * 4, y: 12 + index * 8 },
    html: `<img src="${api.html(src)}" alt="" style="width:${Number(settings.width ?? 220)}px;height:auto" />`
  }));
});
```

The `id` must stay stable for each rendered item because positions are saved by `moduleId:id`. With uploads, index-based ids are fine as long as removing an earlier image is allowed to shift later item positions.

## Registration API

- `setName(name)`: Display name in the overlay settings panel.
- `setDescription(description)`: Help text for the selected overlay.
- `setLocked(true)`: Prevent layout dragging and snap settings.
- `setDefaultPosition({ x, y })`: Default top-left percentage position.
- `setDefaultSettings(settings)`: Default setting values.
- `setWrapperClass(className)`: Classes applied to the overlay wrapper.
- `setCss(css)`: CSS injected while the module is loaded.
- `register("settings", schema)`: Settings shown in the UI.
- `register("visible", fn)`: Return `false` to hide the overlay.
- `register("derive", fn)`: Compute data passed into render handlers.
- `register("tick", fn)`: Run before visibility/render checks. Good for shortcut state.
- `register("renderOverlay", fn)`: Return an HTML string or an array of `{ id, html, defaultPosition }` items.

## Settings

- `Setting.toggle(key, label, defaultValue)`: true/false checkbox.
- `Setting.color(key, label, defaultValue)`: color picker.
- `Setting.range(key, label, min, max, step, defaultValue)`: slider.
- `Setting.number(key, label, defaultValue, min, max, step)`: numeric input.
- `Setting.key(key, label, defaultValue)`: key capture button. Values look like `Insert`, `Ctrl+Shift+K`, or `Numpad1`.
- `Setting.hotkey(key, label, defaultValue)`: alias for `key`.
- `Setting.image(key, label, defaultValue)`: image upload. Values are `data:image/...` URLs for `<img src>`.
- `Setting.images(key, label, defaultValue)`: multi-image upload. Value is an array of `data:image/...` URLs.
- `Setting.text(key, label, defaultValue)`: single-line text input.
- `Setting.textarea(key, label, defaultValue)`: multi-line text input.
- `Setting.select(key, label, options, defaultValue)`: select menu.
- `Setting.selectMenu(key, label, options, defaultValue)`: alias for `select`.

## Handler Arguments

Every handler receives `{ context, data, settings, config, api }`.

- `context`: live overlay state such as `elapsedSeconds`, `lcstats`, `streamOverlays`, `leaderboard`, `endSummary`, `editMode`, and `controlsOpen`.
- `settings`: saved values for this overlay id.
- `data`: the current derived value. `derive` can replace it before render.
- `config`: full overlay config.
- `api`: formatting, data access, CSS class, and input helpers.

Common helpers:

```js
register("renderOverlay", ({ api }) => {
  const stats = api.getLcStats();
  const stream = api.getStreamOverlay();
  const moon = stream?.moonName ?? api.valueAt(stats, "MoonInfo.Name", "Unknown");
  const quota = stream?.quotaValue ?? api.valueAt(stats, "QuotaInfo.NewQuota", 0);

  return `<div>${api.html(moon)}</div><div>${api.number(quota)}</div>`;
});
```

- `api.getLcStats()`: latest LCStatsTracker payload view.
- `api.getLcStatsRaw()`: raw latest LCStatsTracker payload string, or `null`.
- `api.getStreamOverlay()`: latest StreamOverlays payload, or `null`.
- `api.valueAt(root, path, fallback)`: read a nested value.
- `api.valueAtAny(root, paths, fallback)`: read the first matching nested value.
- `api.number(value)`: numeric display formatting.
- `api.html(value)`: HTML escaping.
- `api.className(name)`: stable class name scoped to the overlay id.

StreamOverlays reading is opt-in. Enable `Use StreamOverlays API` in Overlay Settings when a module needs it.

## Input

Use `Setting.key(...)` for configurable shortcuts and read them in `tick`, `visible`, `derive`, or `renderOverlay`.

```js
let enabled = true;

register("settings", [
  Setting.key("toggleKey", "Toggle Key", "Ctrl+K")
]);

register("tick", ({ settings, api }) => {
  if (api.input.consumePress(settings.toggleKey)) enabled = !enabled;
});

register("visible", () => enabled);
```

- `api.input.consumePress(shortcut)`: one-shot key down check.
- `api.input.consumeRelease(shortcut)`: one-shot key release check.
- `api.input.down(shortcut)` / `held(shortcut)` / `shortcut(shortcut)`: true while held.
- `api.input.pressed(shortcut)` / `released(shortcut)`: transient recent checks.
- `api.input.events()`: recent normalized input events.

## Leaderboard Data

`context.leaderboard` contains the current HighQuotaHQ lookup state:

- `status`: `"idle" | "waiting" | "loading" | "ready" | "error"`.
- `collections.vanilla`: `leaderboards_hq`, `leaderboards_sdc`, `leaderboards_smhq`.
- `collections.modded`: `modded_hq`, `modded_sdc`, `modded_smhq`.
- `collections.legacyModded`: legacy modded collection names.
- `boardTypes`: `hq`, `sdc`, `smhq` with display metadata.
- Common ready values: `score`, `rank`, `totalRecords`, `top`, `next`, `nextScore`, `track`, `collectionName`, `boardType`, `version`, and `moon`.
