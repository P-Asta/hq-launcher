# LCStatsTracker Custom Layout

Custom Layout lets LCStatsTracker write run stats into your own Google Sheet
columns instead of using one fixed sheet format.

This document explains the settings in the launcher, what to type into each
field, and what gets written to the sheet.

## Basic Idea

Each column setting is a Google Sheets column letter.

For example:

| Setting | Example input | Meaning |
| --- | --- | --- |
| `Moon` | `F` | Write the moon name into column F |
| `Weather` | `G` | Write the weather into column G |
| `Death state columns` | `AC,AD,AE,AF` | Write player states across columns AC to AF |

Do not include row numbers. Use `F`, not `F3`.

If you leave a column field empty, Custom Layout will not write that stat.
Column letters are normalized automatically, so `aa`, `AA`, and `A A` become
`AA`.

## Quick Start

1. Enable LCStatsTracker in the launcher.
2. Open the LCStatsTracker AutoSheet settings.
3. Select your spreadsheet and sheet tab.
4. Set `Layout` to `Custom Layout`.
5. In `Rows`, set:
   - `Start row`: the first row where run data should be written.
   - `Check column`: a column that is filled for completed rows, usually your moon or collected-value column.
6. Fill only the column fields you want.
7. Run a day. AutoSheet writes to the first empty row at or below `Start row`.

The Custom Layout panel has `Copy` and `Load` buttons. `Copy` copies your full
custom layout JSON. `Load` reads a copied custom layout JSON from the clipboard.

## Recommended Minimal Setup

This is a small useful layout:

| Setting | Input |
| --- | --- |
| `Start row` | `3` |
| `Check column` | `F` |
| `Quota` | `B` |
| `Moon` | `F` |
| `Weather` | `G` |
| `Layout` | `H` |
| `Item count` | `I` |
| `Collected` | `O` |
| `Available` | `P` |
| `Missing` | `Q` |
| `Sold` | `X` |
| `SID` | `Y` |
| `Death state columns` | `AC,AD,AE,AF` |

With that setup, row 3 might look like this after one run:

| B | F | G | H | I | O | P | Q | X | Y | AC | AD | AE | AF |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `900` | `Artifice` | `Clear` | `Mineshaft` | `34` | `926` | `2133` | `1` | `130` | `true` | `S` | `X` | `X` | `X` |

If `SID note` is on, the `Y` cell can also have a note like `Cash register`.

## Rows

| Setting | What it does |
| --- | --- |
| `Start row` | First row AutoSheet may write into. |
| `Check column` | Used to find the first empty row. Pick a column that is usually filled on every run. |
| `Text case` | Changes text case for moon, weather, and interior layout. |
| `Time format` | Changes time display for takeoff time, meteor notes, death notes, and death enemy notes. |

`Text case` options:

| Option | Example output |
| --- | --- |
| `Original` | `Artifice` |
| `UPPERCASE` | `ARTIFICE` |
| `lowercase` | `artifice` |
| `Title Case` | `Mineshaft` |
| `camelCase` | `mineshaft` |
| `PascalCase` | `Mineshaft` |

`Time format` options:

| Option shown | Output |
| --- | --- |
| `7:40 AM` | `7:40 AM` |
| `7:40AM` | `7:40AM` |
| `19:40` | `19:40` |

## Run

| Setting | Writes |
| --- | --- |
| `Quota` | New quota. Economy moons can also update quota. |
| `Seed` | Run seed, or `X` if missing. |
| `Moon` | Moon name without the moon number prefix. |
| `Weather` | Weather. `Mild` is written as `Clear`. |
| `Layout` | Interior layout, such as `Facility` or `Mineshaft`. |
| `Item count` | Dungeon item count. |
| `Apparatus` | `true` or `false` for apparatus spawned. |
| `App less` | Only applies to Facility. Writes `true` when no apparatus spawned. |

## Scrap

| Setting | Writes |
| --- | --- |
| `Bee amount` | Hive count. If `Split count` is on, writes `cheap_count|exp_count`, for example `1|2`. |
| `Split count` | Splits hive counts by value: under 100 is cheap, 100 or higher is expensive. |
| `Bee collected` | Collected hive count. On newer stats, writes `cheap_collected|exp_collected`. |
| `Collected bee value` | Total value of collected hives. |
| `Collected bee note` | Adds a note listing collected hive values. |
| `Bee value` | Available hive values as `cheap_value|exp_value`, for example `64|132`. |
| `Cheap hive` | Cheapest available hive below 100. |
| `Exp hive` | Most expensive available hive at 100 or higher. |
| `hive zero` | Writes `0` for missing hive fields instead of leaving them blank. |
| `Egg` | Available egg values joined by `|`, for example `12|18|30`. |
| `Egg price note` | Adds a note listing available egg values. |
| `Collected egg value` | Total collected egg value. |
| `Collected egg note` | Adds a note listing collected egg values. |
| `Collected` | Collected total value. |
| `Available` | Initial available value. |
| `Real available` | Total available value. |
| `Collected no extra` | Collected value excluding extra values. |
| `Missing` | Count of missed items, with a note listing item names and values. |
| `Gift filter` | Removes collected gift scrap from the missing-items note when possible. |
| `Lost scrap` | Value of scrap lost from previous-day collected items. |
| `Outside items` | Total collected outside item value, with a note for missing bees or eggs. |
| `Sold` | Sold value. Economy moons can also update this. |
| `Gifts` | Gift box summary. See below. |
| `Gift net only` | Writes only net gift value instead of `collected_count|net_value`. |

Gift examples:

| Gift mode | Cell value | Note |
| --- | --- | --- |
| `Gift net only` off | `2|+136` | Every gift box, new scrap value, box value, collected state |
| `Gift net only` on | `+136` | Missed gift boxes only |

## Events

| Setting | Writes |
| --- | --- |
| `Nutcracker` | Nutcracker indoor spawn count. |
| `Nut collect` | Collected shotgun count. |
| `Nut note` | Adds missed shotgun values as a note on the `Nut collect` cell. |
| `Butler` | Butler indoor spawn count. |
| `Butler collect` | Collected knife count. |
| `Butler note` | Adds missed knife values as a note on the `Butler collect` cell. |
| `SID` | `true` when SID happened. Blank unless `SID false` is on when SID did not happen. |
| `SID note` | Adds the SID item name as a note on the `SID` cell. |
| `SID false` | Writes `false` into the `SID` column when SID did not happen. |
| `SID item` | Writes the SID item name as a normal cell value, only when SID happened. |
| `Infes` | `true` when infestation happened, with infestation type as note. |
| `Infes false` | Writes `false` when infestation did not happen. |
| `Fog` | `true` when indoor fog happened. |
| `Fog false` | Writes `false` when indoor fog did not happen. |
| `Meteor` | `true` when meteor happened, with meteor time as note. |
| `Meteor false` | Writes `false` when meteor did not happen. |
| `Take off time` | Ship takeoff time. |
| `Turrets` | Turret count. |
| `Landmines` | Landmine count. |
| `Spiketraps` | Spiketrap count. |

SID example:

| Setting | Input |
| --- | --- |
| `SID` | `Y` |
| `SID note` | on |
| `SID item` | `AA` |

If the SID item was `Cash register`, the sheet gets:

| Y | AA |
| --- | --- |
| `true` with note `Cash register` | `Cash register` |

If no SID happened, `AA` is left blank. `Y` is also blank unless `SID false` is on.

## Enemy

The Enemy section has two kinds of enemy columns:

| Kind | Behavior |
| --- | --- |
| true/false enemies | Write `true` when at least one spawned. Leave blank when none spawned, unless `Write false` is on. |
| count enemies | Write the spawn count. Leave blank when none spawned, unless `Write 0` is on. |

Enemy checks use the code-side enemy name in parentheses. The display name is
for humans; the code name is what LCStatsTracker receives from the game.
Custom Layout checks `IndoorSpawns`, `DayTimeSpawns`, and `NightTimeSpawns`.

| Setting | Code name | Type |
| --- | --- | --- |
| `Jester (Jester)` | `Jester` | true/false |
| `Barber (ClaySurgeon)` | `ClaySurgeon` | true/false |
| `Bunker Spider (SandSpider)` | `SandSpider` | true/false |
| `Bracken (Flowerman)` | `Flowerman` | true/false |
| `Cadaver (Cadaver Growth)` | `Cadaver Growth` | true/false |
| `Ghost Girl (Girl)` | `Girl` | true/false |
| `Maneater (CaveDweller)` | `CaveDweller` | true/false |
| `Backwater Gunkfish (Stingray)` | `Stingray` | count |
| `Coil Head (Spring)` | `Spring` | count |
| `Hoarding Bug (Hoarding Bug)` | `Hoarding Bug` | count |
| `Masked (MaskedPlayerEnemy)` | `MaskedPlayerEnemy` | count |
| `Snare Flea (Centipede)` | `Centipede` | count |
| `Spore Lizard (Puffer)` | `Puffer` | count |
| `Thumper (Crawler)` | `Crawler` | count |

Enemy example:

| Setting | Input |
| --- | --- |
| `Bracken (Flowerman)` | `BA` |
| `Coil Head (Spring)` | `BB` |
| `Snare Flea (Centipede)` | `BC` |
| `Write false` | off |
| `Write 0` | on |

If the run had one Bracken, two Coil Heads, and no Snare Fleas:

| BA | BB | BC |
| --- | --- | --- |
| `true` | `2` | `0` |

### Death Enemy Note

`Death enemy note` adds monster spawn information to player death notes.

This is different from `Death reason notes`:

| Option | Note content |
| --- | --- |
| `Death reason notes` | Time of death and cause of death. |
| `Death enemy note` | Enemies that spawned before that player's death time. |

If both are on, the death reason block comes first, then the enemy spawn block.

Example note:

```text
Time of Death: 10:00 PM
Cause of Death: Blunt force trauma

Inside spawns before death:
Flowerman - 9:00 PM

Night outside spawns before death:
MaskedPlayerEnemy - 9:30 PM / died 9:45 PM
```

Only spawns at or before the player's death time are included.

## Players

| Setting | Writes |
| --- | --- |
| `Death state columns` | Player state values, one player per column. |
| `Player name columns` | Player names, one player per column. |
| `Player name row` | Row used for player names. |
| `Alive value` | Value for alive players. |
| `Dead value` | Value for dead players. |
| `Late death` | Value for deaths at or after 10:45 PM, if set. |
| `Missing value` | Value for abandoned/missing players. |
| `Disconnected value` | Value for disconnected players. |
| `Death reason notes` | Adds time and cause of death as a note on the death state cell. |
| `Names as notes` | Puts player names as notes instead of normal cell values. |

Player example:

| Setting | Input |
| --- | --- |
| `Death state columns` | `AC,AD,AE,AF` |
| `Player name columns` | `AC,AD,AE,AF` |
| `Player name row` | `1` |
| `Alive value` | `S` |
| `Dead value` | `X` |

Player names are written to row 1, and each run writes player states to the run
row. If a death note is needed, the state cell is written with a note.

## Notes And Blank Values

Some fields write notes to the same cell as the value. Examples:

| Field | Cell value | Note |
| --- | --- | --- |
| `Missing` | missed item count, or `X` | missed item names and values |
| `Outside items` | collected outside value, or `X` | missing bees/eggs |
| `Nut collect` | collected shotgun count | missed shotgun values, if `Nut note` is on |
| `Butler collect` | collected knife count | missed knife values, if `Butler note` is on |
| `SID` | `true` or `false` | SID item name if `SID note` is on |
| `Infes` | `true` or `false` | infestation type |
| `Meteor` | `true` or `false` | meteor time |
| `Death state columns` | player state | death reason and/or enemy spawn info |

When a setting is disabled by leaving its column blank, Custom Layout does not
touch that column. When a note-capable cell is written without a note, stale
notes in that cell are cleared.

## Example Full Preset JSON

You can paste this into the clipboard and use `Load` in the Custom Layout panel.
Adjust the columns to match your sheet.

```json
{
  "startRow": 3,
  "checkColumn": "F",
  "textCase": "Original",
  "timeFormat": "12-hour",
  "quotaColumn": "B",
  "seedColumn": "",
  "moonColumn": "F",
  "weatherColumn": "G",
  "layoutColumn": "H",
  "itemCountColumn": "I",
  "apparatusColumn": "",
  "beeAmountColumn": "J",
  "splitHiveCount": true,
  "beehiveCollectedColumn": "",
  "beehiveCollectedValueColumn": "",
  "beehiveCollectedNotesEnabled": true,
  "beeValueColumn": "K",
  "cheapHiveColumn": "",
  "expensiveHiveColumn": "",
  "writeZeroForMissingHives": false,
  "eggColumn": "L",
  "eggNotesEnabled": false,
  "collectedEggColumn": "",
  "collectedEggNotesEnabled": true,
  "nutColumn": "M",
  "nutCollectColumn": "",
  "nutNotesEnabled": false,
  "butlerColumn": "N",
  "butlerCollectColumn": "",
  "butlerNotesEnabled": false,
  "collectedColumn": "O",
  "availableColumn": "P",
  "realAvailableColumn": "",
  "collectedNoExtraColumn": "",
  "missingColumn": "Q",
  "filterCollectedGiftScrapFromMissing": true,
  "outsideItemsColumn": "",
  "soldColumn": "X",
  "sidColumn": "Y",
  "sidNotesEnabled": true,
  "sidWriteFalse": false,
  "sidItemColumn": "AA",
  "infestationColumn": "Z",
  "infestationWriteFalse": false,
  "lostScrapColumn": "AB",
  "takeoffTimeColumn": "",
  "turretColumn": "",
  "landmineColumn": "",
  "spiketrapColumn": "",
  "appLessColumn": "",
  "deathColumns": "AC,AD,AE,AF",
  "playerNameColumns": "AC,AD,AE,AF",
  "playerNameRow": 1,
  "aliveState": "S",
  "deadState": "X",
  "missingState": "M",
  "disconnectedState": "DC",
  "lateDeadState": "SX",
  "deathNotesEnabled": true,
  "playerNamesAsNotes": false,
  "deathEnemyNotesEnabled": true,
  "enemyWriteFalse": false,
  "enemyWriteZero": true,
  "jesterColumn": "AG",
  "barberColumn": "AH",
  "bunkerSpiderColumn": "AI",
  "brackenColumn": "AJ",
  "cadaverColumn": "",
  "ghostGirlColumn": "",
  "maneaterColumn": "",
  "backwaterGunkfishColumn": "AK",
  "coilHeadColumn": "AL",
  "hoardingBugColumn": "AM",
  "maskedColumn": "AN",
  "snareFleaColumn": "AO",
  "sporeLizardColumn": "AP",
  "thumperColumn": "AQ",
  "fogColumn": "AR",
  "fogWriteFalse": false,
  "meteorColumn": "AS",
  "meteorWriteFalse": false,
  "giftsColumn": "AT",
  "giftBoxesNetOnly": false
}
```

## Troubleshooting

| Problem | Check |
| --- | --- |
| Nothing writes | Make sure `Layout` is `Custom Layout`, spreadsheet and sheet tab are selected, and Google login is active unless using no-Google mode. |
| Data writes to the wrong row | Check `Start row` and `Check column`. The check column should be filled on rows that already have data. |
| A stat is missing | Make sure its column field is not empty. Empty column fields disable that stat. |
| `false` or `0` is not written | Enable the matching `Write false`, `Write 0`, `SID false`, `Infes false`, `Fog false`, or `Meteor false` option. |
| Player notes are missing | Enable `Death reason notes` and/or `Death enemy note`. Missing/abandoned players do not get death notes. |
| Enemy column does not count | Use the provided field for that enemy. The code name in parentheses is what the writer checks internally. |
