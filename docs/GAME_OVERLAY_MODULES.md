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
setDescription("Shows normalized remaining scrap after a run ends.");
setDefaultPosition({ x: 8, y: 10 });
setWrapperClass("rounded border border-white/15 bg-black/70 p-3");

register("settings", [
  Setting.toggle("enabled", "Enabled", true),
  Setting.color("color", "Color", "#ffffff")
]);

register("visible", ({ context, settings }) => context.editMode || settings.enabled !== false);

register("renderOverlay", ({ api, settings }) => {
  const scrap = api.scrap.summary();
  const total = scrap.remaining ?? 0;
  return `<div style="color:${api.html(settings.color)}">${api.html(scrap.moon)} Scrap ${api.number(total)}</div>`;
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
  const scrap = api.scrap.summary();
  const bracken = api.enemies.counts("Bracken");
  const eyelessDogs = api.enemies.counts("Eyeless Dog");

  return `<div>${api.html(scrap.moon)}</div>
    <div>Scrap ${api.number(scrap.remaining ?? 0)}</div>
    <div>Bracken ${api.number(bracken.spawned)}</div>
    <div>Eyeless Dog ${api.number(eyelessDogs.spawned)}</div>`;
});
```

- `api.getLcStats()`: latest LCStatsTracker payload view.
- `api.getLcStatsRaw()`: raw latest LCStatsTracker payload string, or `null`.
- `api.getStreamOverlay()`: latest StreamOverlays payload, or `null`.
- `api.scrap.summary()`: normalized end-run scrap data from LCStatsTracker/end-summary sources.
- `api.scrap.items()`: missed/remaining scrap as `{ name, value, raw }[]`.
- `api.scrap.groups()`: missed/remaining scrap grouped by name.
- `api.scrap.remaining()` / `api.scrap.total()`: normalized remaining scrap value, or `null`.
- `api.scrap.moon()`: normalized moon name.
- `api.enemies.list()`: typed Custom Layout enemy catalog with display names, code names, type, and default source.
- `api.enemies.counts(name, options)`: normalized `{ killed, spawned, alive, present }` counts for any typed catalog display name or code name.
- `api.enemies.spawned(name, options)` / `killed` / `alive` / `present`: direct count helpers.
- `api.enemies.butler()` / `api.enemies.nutcracker()`: convenience shortcuts.
- `api.number(value)`: numeric display formatting.
- `api.html(value)`: HTML escaping.
- `api.className(name)`: stable class name scoped to the overlay id.

StreamOverlays reading is opt-in. Enable `Use StreamOverlays API` in Overlay Settings when a module needs it.

### Normalized Scrap API

Use `api.scrap` when you want end-run missed/remaining scrap without reading
LCStatsTracker field names yourself. All scrap helpers read the same normalized
summary object.

`api.scrap.summary()` returns an `OverlayScrapSummary`:

```js
{
  moon: "Artifice",
  total: 455,
  remaining: 455,
  items: [
    { name: "Cash Register", value: 160, raw: {} },
    { name: "Gold Bar", value: 110, raw: {} }
  ],
  groups: [
    {
      name: "Apparatus",
      values: [30, 20, 10],
      total: 60,
      max: 30,
      count: 3,
      items: [
        { name: "Apparatus", value: 30, raw: {} },
        { name: "Apparatus", value: 20, raw: {} },
        { name: "Apparatus", value: 10, raw: {} }
      ]
    }
  ]
}
```

`OverlayScrapSummary` fields:

| Field | Type | Meaning |
| --- | --- | --- |
| `moon` | `string` | Normalized moon/run name. Falls back to `"Unknown"`. |
| `total` | `number \| null` | Normalized scrap left behind. Same value as `remaining`. |
| `remaining` | `number \| null` | Scrap left behind. `null` when no reliable source exists. |
| `items` | `OverlayScrapItem[]` | Individual missed/remaining scrap items, sorted by `value` high to low. |
| `groups` | `OverlayScrapGroup[]` | `items` grouped by normalized item `name`. |

`OverlayScrapItem` fields:

| Field | Type | Meaning |
| --- | --- | --- |
| `name` | `string` | Human display name such as `"Cash Register"` or `"Gold Bar"`. |
| `value` | `number` | Scrap value. Missing or unparsable values become `0`. |
| `raw` | `any` | Original payload item. Use only for debugging or advanced modules. |

`OverlayScrapGroup` fields:

| Field | Type | Meaning |
| --- | --- | --- |
| `name` | `string` | Shared item name for the group. |
| `values` | `number[]` | All values in the group, sorted high to low. |
| `total` | `number` | Sum of all `values`. |
| `max` | `number` | Highest individual value in the group. |
| `count` | `number` | Number of items in the group. |
| `items` | `OverlayScrapItem[]` | The individual normalized items in this group. |

`groups` are sorted by `max` descending, then `total` descending, then `name`
ascending. That means the most important high-value groups naturally render
first.

Scrap helpers:

| Helper | Returns |
| --- | --- |
| `api.scrap.moon()` | `string`, usually the moon/run title or `"Unknown"` |
| `api.scrap.remaining()` | `number | null`, normalized scrap left behind |
| `api.scrap.total()` | `number | null`, alias for `remaining()` |
| `api.scrap.items()` | `OverlayScrapItem[]`, sorted high value first |
| `api.scrap.groups()` | `OverlayScrapGroup[]`, grouped by item name |

Common scrap patterns:

```js
const scrap = api.scrap.summary();
const totalText = scrap.remaining == null ? "?" : api.number(scrap.remaining);

const rows = scrap.groups.slice(0, 6).map((group) => {
  const detail = group.count > 1 ? `${group.count}x / ${api.number(group.total)}` : api.number(group.max);
  return `<div>${api.html(group.name)} ${detail}</div>`;
}).join("");
```

### Normalized Enemy API

Use `api.enemies` when you want spawn/killed counts without manually checking
`IndoorSpawns`, `DayTimeSpawns`, or `NightTimeSpawns`.

`api.enemies.counts("Bracken")` returns an `OverlayEnemyCounts`:

```js
{
  id: "bracken",
  name: "Bracken",
  names: ["Flowerman"],
  kind: "bool",
  source: "all",
  killed: 0,
  spawned: 1,
  alive: 1,
  present: true
}
```

`OverlayEnemyCounts` fields:

| Field | Type | Meaning |
| --- | --- | --- |
| `id` | `string` | Stable catalog id, such as `"bracken"` or `"eyelessDog"`. |
| `name` | `string` | Human display name, such as `"Bracken"`. |
| `names` | `string[]` | LCStatsTracker code names and aliases matched for this enemy. |
| `kind` | `"bool" \| "count" \| string` | Custom Layout style. `bool` means presence is usually displayed; `count` means count is usually displayed. |
| `source` | `OverlayEnemySource` | Spawn group used for this count. |
| `killed` | `number` | Best-effort killed count from available payloads. |
| `spawned` | `number` | Spawn count from the selected source. |
| `alive` | `number` | `Math.max(0, spawned - killed)`. |
| `present` | `boolean` | `true` when `spawned > 0`. |

`OverlayEnemyDefinition`, returned by `api.enemies.list()`:

| Field | Type | Meaning |
| --- | --- | --- |
| `id` | `string` | Stable catalog id accepted by `api.enemies.counts(id)`. |
| `name` | `string` | Human display name. |
| `names` | `string[]` | LCStatsTracker code names and aliases. |
| `kind` | `"bool" \| "count" \| string` | Suggested display style. |
| `source` | `"all" \| "indoor" \| "night" \| string` | Default source used by `counts()`. |

`OverlayEnemySource` values:

| Source | Spawn arrays read |
| --- | --- |
| `"all"` | `IndoorSpawns`, `DayTimeSpawns`, and `NightTimeSpawns` |
| `"indoor"` / `"inside"` | `IndoorSpawns` only |
| `"day"` / `"daytime"` | `DayTimeSpawns` only |
| `"night"` / `"nighttime"` / `"outside"` | `NightTimeSpawns` only |

Enemy helpers:

| Helper | Returns |
| --- | --- |
| `api.enemies.list()` | `OverlayEnemyDefinition[]`, every known typed enemy entry. |
| `api.enemies.counts(name, options)` | `OverlayEnemyCounts` |
| `api.enemies.spawned(name, options)` | `number` |
| `api.enemies.killed(name, options)` | `number` |
| `api.enemies.alive(name, options)` | `number` |
| `api.enemies.present(name, options)` | `boolean`, true when `spawned > 0` |
| `api.enemies.butler(options)` | `OverlayEnemyCounts` for Butler |
| `api.enemies.nutcracker(options)` | `OverlayEnemyCounts` for Nutcracker |

Common enemy patterns:

```js
const bracken = api.enemies.counts("Bracken");
const coilHeads = api.enemies.spawned("Coil Head");
const outsideDogs = api.enemies.counts("Eyeless Dog", { source: "night" });

const visibleEnemies = api.enemies.list()
  .map((enemy) => api.enemies.counts(enemy))
  .filter((enemy) => enemy.present);
```

Scrap/enemy helpers let modules avoid hard-coding LCStatsTracker field names:

```js
register("derive", ({ api }) => {
  const scrap = api.scrap.summary();
  const enemies = ["Jester", "Flowerman", "Spring", "MouthDog"].map((name) => ({
    name,
    ...api.enemies.counts(name)
  }));
  return {
    moon: scrap.moon,
    total: scrap.remaining,
    groups: scrap.groups,
    enemies
  };
});
```

Enemy names follow the Custom Layout catalog. You can use either display names like
`"Bracken"` and `"Eyeless Dog"` or LCStatsTracker code names like `"Flowerman"`
and `"MouthDog"`. Pass `{ source: "indoor" }`, `{ source: "day" }`, or
`{ source: "night" }` when you need a specific spawn group.

Known names are typed for autocomplete, including indoor, daytime, and nighttime
entities such as `"Hygrodere"`, `"MaskedPlayerEnemy"`, `"Tulip Snake"`,
`"Giant Sapsucker"`, `"Old Bird"`, and `"Kidnapper Fox"`. For modded enemies,
pass a custom query object:

```js
api.enemies.counts({ name: "Custom Monster", names: ["CustomMonster"] });
```

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

On Windows, module shortcuts are observed without blocking the original key
input, so a key bound by an overlay module should still reach Lethal Company.

The same shortcut can be assigned to multiple module settings. When one module
uses the same key for multiple independent actions, pass a stable scope string
so each binding can consume the same physical key press once:

```js
register("tick", ({ settings, api }) => {
  if (api.input.consumePress(settings.toggleKey, "toggle")) {
    enabled = !enabled;
  }
  if (api.input.consumePress(settings.dismissKey, "dismiss")) {
    dismissed = true;
  }
});
```

## Leaderboard Data

`context.leaderboard` contains the current HighQuotaHQ lookup state:

- `status`: `"idle" | "waiting" | "loading" | "ready" | "error"`.
- `collections.vanilla`: `leaderboards_hq`, `leaderboards_sdc`, `leaderboards_smhq`.
- `collections.modded`: `modded_hq`, `modded_sdc`, `modded_smhq`.
- `collections.legacyModded`: legacy modded collection names.
- `boardTypes`: `hq`, `sdc`, `smhq` with display metadata.
- Common ready values: `score`, `rank`, `totalRecords`, `top`, `next`, `nextScore`, `track`, `collectionName`, `boardType`, `version`, and `moon`.
