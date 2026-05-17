"""
CharlyAutoSheet라는 레이아웃을 만들꺼야 charly.py처럼 쓰여지며, 저 코드는 위치를 가져와 하는 비 효율적인 방식임으로 위치를 고정하고 한번에 채우는 wafrody, maku 등등의 레이아웃과 같은 방식으로 좀 더 최적화 할꺼야


QUOTA AMOUNT: B
MOON: F
WEATHER: G
LAYOUT: H
ITEM COUNT: I
Bee: J K
Egg: L
Nut: M
But:  N
COLLECTED: O
AVAILABLE: P
Missing: Q
SOLD: X
SID: Y
INFES: Z
LOST SCRAP: AB
DEATHS: AC, AD, AE, AF
FOG: AG
METEOR: AH
GIFTS: AI


"""

import os
import json
import time
import urllib.request
import urllib.error
import configparser
from googleapiclient.discovery import build
from google.oauth2.service_account import Credentials


BASE_DIR = os.path.dirname(os.path.abspath(__file__))
config = configparser.ConfigParser()
config_path = os.path.join(BASE_DIR, "config.ini")

if not os.path.exists(config_path):
    raise FileNotFoundError(f"config.ini not found at {config_path}")

config.read(config_path)

json_file_name = config.get("GoogleSheets", "json_file_name")
SPREADSHEET_ID = config.get("GoogleSheets", "spreadsheet_id")
target_sheet   = config.get("GoogleSheets", "target_sheet")

START_ROW = config.getint("Sheet", "start_row")

PLAYER_COLUMNS = [c.strip() for c in config.get("Columns", "Players").split(",") if c.strip()]

COLUMN_MAP = {
    "NewQuota":             config.get("Columns", "NewQuota"),
    "MoonInfo_Name":        config.get("Columns", "MoonInfo_Name"),
    "MoonInfo_Weather":     config.get("Columns", "MoonInfo_Weather"),
    "DungeonInfo_Interior": config.get("Columns", "DungeonInfo_Interior"),
    "DungeonInfo_ItemCount":config.get("Columns", "DungeonInfo_ItemCount"),
    "BeehiveAmount":        config.get("Columns", "BeehiveAmount"),
    "BeehiveValue":         config.get("Columns", "BeehiveValue"),
    "EggValue":             config.get("Columns", "EggValue"),
    "CollectedTotal":       config.get("Columns", "CollectedTotal"),
    "BottomLine":           config.get("Columns", "BottomLine"),
    "MissedItems":          config.get("Columns", "MissedItems"),
    "ValueSold":            config.get("Columns", "ValueSold"),
    "SIDType":              config.get("Columns", "SID"),
    "InfestationType":      config.get("Columns", "Infestation"),
    "IndoorFog":            config.get("Columns", "IndoorFog"),
    "MeteorShower":         config.get("Columns", "MeteorShower"),
    "GiftBoxes":            config.get("Columns", "GiftBoxes", fallback=None),
    "Seed":                 config.get("Columns", "Seed"),
}

CHECKBOX_FIELDS = {"SIDType", "InfestationType", "IndoorFog", "MeteorShower"}

SERVICE_ACCOUNT_FILE = os.path.join(BASE_DIR, "extra", json_file_name)
SCOPES = ["https://www.googleapis.com/auth/spreadsheets"]
creds = Credentials.from_service_account_file(SERVICE_ACCOUNT_FILE, scopes=SCOPES)
service = build("sheets", "v4", credentials=creds)

print(f"Target sheet: '{target_sheet}'")

STATS_URL = os.getenv("STATS_URL", "http://localhost:2145/")
FALLBACK_STATS_FILE = os.path.join(
    os.path.expanduser("~"),
    "Documents",
    "LethalCompanyStats",
    "stats.json"
)


def col_letter_to_index(col: str) -> int:
    col = col.upper()
    index = 0
    for ch in col:
        index = index * 26 + (ord(ch) - ord("A") + 1)
    return index - 1


def sorted_column_map_keys(column_map: dict) -> list[str]:
    def sort_key(k):
        col = column_map[k]
        if col is None:
            return (10 ** 9,)
        return (col_letter_to_index(col),)
    return sorted(column_map.keys(), key=sort_key)


def parse_sse_payload(raw_text):
    lines = [line.strip() for line in raw_text.splitlines() if line.strip()]
    data_lines = []
    for line in lines:
        if line.startswith("data:"):
            data_lines.append(line[len("data:"):].strip())
    return "\n".join(data_lines)


def get_stats_from_http():
    try:
        with urllib.request.urlopen(STATS_URL, timeout=5) as response:
            raw = response.read().decode("utf-8")
        raw = raw.lstrip("\ufeff")
        if not raw.strip():
            return None
        try:
            return json.loads(raw)
        except json.JSONDecodeError:
            payload = parse_sse_payload(raw)
            if not payload.strip():
                return None
            return json.loads(payload)
    except Exception:
        return None


def get_stats_from_file():
    if not os.path.exists(FALLBACK_STATS_FILE):
        return None
    try:
        with open(FALLBACK_STATS_FILE, "r", encoding="utf-8") as f:
            return json.load(f)
    except Exception:
        return None


def get_stats():
    stats = get_stats_from_http()
    if stats is not None:
        return stats
    return get_stats_from_file()


def strip_apostrophe(value):
    return str(value).lstrip("'")


def strip_moon_number(name: str) -> str:
    parts = name.split(" ", 1)
    if len(parts) == 2 and parts[0].rstrip("-").isdigit():
        return parts[1]
    return name


def coerce_value(value):
    if isinstance(value, bool):
        return value
    try:
        return int(value)
    except (ValueError, TypeError):
        pass
    try:
        return float(value)
    except (ValueError, TypeError):
        pass
    return value


def make_cell_value(value):
    if isinstance(value, bool):
        return {"boolValue": value}
    try:
        return {"numberValue": int(value)}
    except (ValueError, TypeError):
        pass
    try:
        return {"numberValue": float(value)}
    except (ValueError, TypeError):
        pass
    return {"stringValue": str(value)}


def normalize_players(raw_players: dict) -> list[dict]:
    players = []
    for steam_id, data in raw_players.items():
        alive          = data.get("Alive", False)
        disconnected   = data.get("Disconnected", False)
        time_of_death  = strip_apostrophe(data.get("TimeOfDeath", "")).strip()
        cause_of_death = strip_apostrophe(data.get("CauseOfDeath", "")).strip()

        if disconnected:
            status = "DC"
        elif cause_of_death.lower() in ("abandonment", "abandoned"):
            status = "M"
        elif alive:
            status = "S"
        else:
            status = "X"

        note_parts = []
        if time_of_death:
            note_parts.append(f"Time of Death: {time_of_death}")
        if cause_of_death:
            note_parts.append(f"Cause of Death: {cause_of_death}")
        note = "\n".join(note_parts)

        players.append({"status": status, "note": note})

    return players


def normalize_gift_boxes(raw_gift_boxes: list) -> dict:
    if not raw_gift_boxes:
        return {"amount": 0, "total_value": 0, "cell_value": "", "note": ""}

    collected = [box for box in raw_gift_boxes if box.get("Collected", False)]

    amount = len(collected)
    total_net = sum(
        int(box.get("GiftValue", 0)) - int(box.get("ScrapValue", 0))
        for box in collected
    )

    sign = "+" if total_net >= 0 else ""
    cell_value = f"{amount}|{sign}{total_net}" if collected else ""

    note_lines = []
    for i, box in enumerate(raw_gift_boxes, start=1):
        gift  = int(box.get("GiftValue", 0))
        scrap = int(box.get("ScrapValue", 0))
        was_collected = box.get("Collected", False)
        note_lines.append(f"Box {i}: GiftValue={gift}, ScrapValue={scrap}, Collected={was_collected}")
    note = "\n".join(note_lines)

    return {"amount": amount, "total_value": total_net, "cell_value": cell_value, "note": note}


def normalize_missed_items(raw_missed_items: list) -> dict:
    if not raw_missed_items:
        return {"total_value": 0, "cell_value": "", "note": ""}

    uncollected = [item for item in raw_missed_items if not item.get("CollectedOnPreviousDay", False)]

    count = len(uncollected)
    cell_value = str(count) if uncollected else ""

    note_lines = []
    for item in uncollected:
        name  = item.get("ItemType", "Unknown")
        value = int(item.get("Value", 0))
        note_lines.append(f"{name}: {value}")
    note = "\n".join(note_lines)

    return {"total_value": count, "cell_value": cell_value, "note": note}


def normalize_stats(stats):
    dungeon      = stats.get("DungeonInfo") or {}
    moon         = stats.get("MoonInfo") or {}
    bee_info     = stats.get("BeeInfo") or {}
    egg_info     = stats.get("EggInfo") or {}
    gift_boxes   = stats.get("GiftBoxes") or []
    missed_items = stats.get("MissedItems") or []

    bee_available = [int(v) for v in (bee_info.get("Available") or [])]
    egg_available = [int(v) for v in (egg_info.get("Available") or [])]

    raw_players = stats.get("Players") or {}
    if not isinstance(raw_players, dict):
        raw_players = {}

    moon_name = strip_moon_number(strip_apostrophe(moon.get("Name", "")))
    weather   = strip_apostrophe(moon.get("Weather", ""))
    if weather == "Mild":
        weather = "Clear"

    indoor_fog_val = "true" if stats.get("IndoorFog", False) else ""

    meteor_time = strip_apostrophe(stats.get("MeteorShowerTime", "")).strip()
    meteor_val  = meteor_time if meteor_time else ""

    bee_small = [v for v in bee_available if v < 100]
    bee_large = [v for v in bee_available if v >= 100]
    if bee_available:
        small_val = bee_small[0] if bee_small else 0
        large_val = bee_large[0] if bee_large else 0
        beehive_amount = f"{len(bee_small)}|{len(bee_large)}"
        beehive_value  = f"{small_val}|{large_val}"
    else:
        beehive_amount = ""
        beehive_value  = ""

    egg_value_str = "|".join(str(v) for v in sorted(egg_available)) if egg_available else ""

    gift_data   = normalize_gift_boxes(gift_boxes)
    missed_data = normalize_missed_items(missed_items)

    return {
        "NewQuota":              int(strip_apostrophe(stats.get("NewQuota", 0))),
        "MoonInfo_Name":         moon_name,
        "MoonInfo_Weather":      weather,
        "DungeonInfo_Interior":  strip_apostrophe(dungeon.get("Interior", "")),
        "DungeonInfo_ItemCount": int(strip_apostrophe(dungeon.get("ItemCount", 0))),
        "BeehiveAmount":         beehive_amount,
        "BeehiveValue":          beehive_value,
        "EggValue":              egg_value_str,
        "CollectedTotal":        int(strip_apostrophe(stats.get("CollectedTotal", 0))),
        "BottomLine":            int(strip_apostrophe(stats.get("BottomLine", 0))),
        "MissedItems":           missed_data,
        "ValueSold":             int(strip_apostrophe(stats.get("ValueSold", 0))),
        "SIDType":               strip_apostrophe(stats.get("SIDType", "")),
        "InfestationType":       strip_apostrophe(stats.get("InfestationType", "")),
        "IndoorFog":             indoor_fog_val,
        "MeteorShower":          meteor_val,
        "GiftBoxes":             gift_data,
        "Seed":                  strip_apostrophe(stats.get("Seed", "")),
        "Players":               normalize_players(raw_players),
    }


def get_sheet_id(sheet_name: str) -> int:
    meta = service.spreadsheets().get(spreadsheetId=SPREADSHEET_ID).execute()
    for sheet in meta.get("sheets", []):
        props = sheet.get("properties", {})
        if props.get("title") == sheet_name:
            return props["sheetId"]
    raise ValueError(f"Sheet '{sheet_name}' not found in spreadsheet")


def get_next_empty_row():
    col = COLUMN_MAP["MoonInfo_Name"]
    result = service.spreadsheets().values().get(
        spreadsheetId=SPREADSHEET_ID,
        range=f"{target_sheet}!{col}{START_ROW}:{col}1000"
    ).execute()
    rows = result.get("values", [])
    return START_ROW + len(rows)


def write_to_cell(value, cell):
    service.spreadsheets().values().update(
        spreadsheetId=SPREADSHEET_ID,
        range=f"{target_sheet}!{cell}",
        valueInputOption="RAW",
        body={"values": [[coerce_value(value)]]}
    ).execute()


def write_cell_with_note(col: str, row: int, value: str, note: str):
    sheet_id  = get_sheet_id(target_sheet)
    col_index = col_letter_to_index(col)
    row_index = row - 1

    service.spreadsheets().batchUpdate(
        spreadsheetId=SPREADSHEET_ID,
        body={"requests": [{
            "updateCells": {
                "range": {
                    "sheetId":          sheet_id,
                    "startRowIndex":    row_index,
                    "endRowIndex":      row_index + 1,
                    "startColumnIndex": col_index,
                    "endColumnIndex":   col_index + 1,
                },
                "rows": [{"values": [{"userEnteredValue": make_cell_value(value), "note": note}]}],
                "fields": "userEnteredValue,note",
            }
        }]}
    ).execute()


def write_checkbox(col: str, row: int, checked: bool):
    sheet_id  = get_sheet_id(target_sheet)
    col_index = col_letter_to_index(col)
    row_index = row - 1

    service.spreadsheets().batchUpdate(
        spreadsheetId=SPREADSHEET_ID,
        body={"requests": [{
            "updateCells": {
                "range": {
                    "sheetId":          sheet_id,
                    "startRowIndex":    row_index,
                    "endRowIndex":      row_index + 1,
                    "startColumnIndex": col_index,
                    "endColumnIndex":   col_index + 1,
                },
                "rows": [{"values": [{"userEnteredValue": {"boolValue": checked}}]}],
                "fields": "userEnteredValue",
            }
        }]}
    ).execute()


def write_checkbox_with_note(col: str, row: int, checked: bool, note: str):
    sheet_id  = get_sheet_id(target_sheet)
    col_index = col_letter_to_index(col)
    row_index = row - 1

    service.spreadsheets().batchUpdate(
        spreadsheetId=SPREADSHEET_ID,
        body={"requests": [{
            "updateCells": {
                "range": {
                    "sheetId":          sheet_id,
                    "startRowIndex":    row_index,
                    "endRowIndex":      row_index + 1,
                    "startColumnIndex": col_index,
                    "endColumnIndex":   col_index + 1,
                },
                "rows": [{"values": [{"userEnteredValue": {"boolValue": checked}, "note": note if checked else ""}]}],
                "fields": "userEnteredValue,note",
            }
        }]}
    ).execute()


def write_players(players: list[dict], row: int):
    if len(players) > len(PLAYER_COLUMNS):
        print(f"⚠ More players ({len(players)}) than configured columns ({len(PLAYER_COLUMNS)}); extras ignored")
    sorted_player_cols = sorted(PLAYER_COLUMNS, key=col_letter_to_index)
    for i, col in enumerate(sorted_player_cols):
        if i >= len(players):
            break
        player = players[i]
        if player["note"]:
            write_cell_with_note(col, row, player["status"], player["note"])
        else:
            write_to_cell(player["status"], f"{col}{row}")


def write_gift_boxes(gift_data: dict, col: str, row: int):
    if not gift_data["note"]:
        write_to_cell("X", f"{col}{row}")
        return
    if gift_data["cell_value"]:
        write_cell_with_note(col, row, gift_data["cell_value"], gift_data["note"])
    else:
        write_cell_with_note(col, row, "X", gift_data["note"])


def write_missed_items(missed_data: dict, col: str, row: int):
    if not missed_data["note"]:
        write_to_cell("X", f"{col}{row}")
        return
    if missed_data["cell_value"]:
        write_cell_with_note(col, row, missed_data["cell_value"], missed_data["note"])
    else:
        write_cell_with_note(col, row, "X", missed_data["note"])


def update_sheet_from_stats(stats):
    normalized = normalize_stats(stats)
    target_row = get_next_empty_row()
    moon_name  = normalized["MoonInfo_Name"]

    if "gordion" in moon_name.lower():
        value_sold = normalized["ValueSold"]
        new_quota  = normalized["NewQuota"]
        if value_sold == 0 and new_quota == 0:
            return
        if value_sold != 0:
            write_to_cell(value_sold, f'{COLUMN_MAP["ValueSold"]}{target_row - 3}')
        if new_quota != 0:
            write_to_cell(new_quota, f'{COLUMN_MAP["NewQuota"]}{target_row}')
        print(f"Updated {target_sheet} (Gordion: sold={value_sold}, quota={new_quota})")
        return

    for key in sorted_column_map_keys(COLUMN_MAP):
        col = COLUMN_MAP[key]
        if col is None:
            continue

        if key == "GiftBoxes":
            write_gift_boxes(normalized["GiftBoxes"], col, target_row)
            continue

        if key == "MissedItems":
            write_missed_items(normalized["MissedItems"], col, target_row)
            continue

        value = normalized[key]

        if key == "IndoorFog":
            write_checkbox(col, target_row, bool(str(value).strip()))
            continue

        if key == "MeteorShower":
            write_checkbox_with_note(col, target_row, bool(str(value).strip()), str(value))
            continue

        if key in ("SIDType", "InfestationType"):
            write_checkbox_with_note(col, target_row, bool(str(value).strip()), str(value))
            continue

        if key in ("ValueSold", "NewQuota") and value == 0:
            continue

        if key == "EggValue" and value == "":
            write_to_cell("X", f"{col}{target_row}")
            continue

        if key in ("BeehiveAmount", "BeehiveValue") and value == "":
            write_to_cell("X", f"{col}{target_row}")
            continue

        write_to_cell(value, f"{col}{target_row}")

    write_players(normalized["Players"], target_row)
    print(f"Updated {target_sheet} (row {target_row})")


def main():
    print(f"Watching for stats — target sheet: '{target_sheet}'")
    last_stats_text = None
    while True:
        try:
            stats = get_stats()
            if stats is not None:
                current_stats_text = json.dumps(stats, sort_keys=True)
                if current_stats_text != last_stats_text:
                    update_sheet_from_stats(stats)
                    last_stats_text = current_stats_text
        except Exception as e:
            print(f"✗ Error: {e}")
        time.sleep(1)


if __name__ == "__main__":
    main()