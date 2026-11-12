#!/usr/bin/python3
"""Keep Steam's authenticated launch and select an independently installed HQ version."""
import json
import os
from pathlib import Path
import re
import sys


APP_ID = "1966720"
DEPOT_ID = "1966721"
STORE = Path("/root/.local/share/hq-launcher")


def checked_path(base, path):
    base, path = Path(base), Path(path)
    try:
        parts = path.relative_to(base).parts
    except ValueError:
        raise ValueError("HQ version path escapes its installation store")
    current = base
    if current.is_symlink():
        raise ValueError("HQ version store is a symbolic link")
    for part in parts:
        if part in ("", ".", ".."):
            raise ValueError("Invalid HQ version path")
        current = current / part
        if current.is_symlink():
            raise ValueError("Symbolic link in HQ version path")
    if not path.resolve().is_relative_to(base.resolve()):
        raise ValueError("HQ version path escapes its installation store")
    return path


def read_json(base, path):
    path = checked_path(base, path)
    if not path.is_file() or path.stat().st_size > 1024 * 1024:
        raise ValueError("Missing or oversized HQ installation metadata")
    try:
        with path.open(encoding="utf-8") as source:
            value = json.load(source)
    except (OSError, ValueError) as error:
        raise ValueError("Invalid HQ installation metadata: %s" % error)
    if not isinstance(value, dict):
        raise ValueError("Invalid HQ installation metadata")
    return value


def prepare_launch(argv, environ, base=STORE):
    """Return (argv, environment, working directory), without starting or modifying anything."""
    argv, environ, base = list(argv), dict(environ), Path(base)
    selection_path = checked_path(base, base / "selection.json")
    if not selection_path.exists():
        return argv, environ, None
    selected = read_json(base, selection_path)
    version, manifest = selected.get("version"), selected.get("depot_manifest")
    if (type(selected.get("format")) is not int or selected.get("format") != 1
            or type(version) is not int or not 1 <= version <= 100000
            or not isinstance(manifest, str) or not re.fullmatch(r"[0-9]{1,32}", manifest)):
        raise ValueError("Invalid selected HQ version")
    if environ.get("SteamAppId") != APP_ID or environ.get("STEAM_COMPAT_APP_ID", APP_ID) != APP_ID:
        raise ValueError("HQ version launch requires Steam's Lethal Company app identity")
    directory = checked_path(base, base / "versions" / ("%s-%s" % (version, manifest)))
    receipt = read_json(base, directory / "install.json")
    if (type(receipt.get("format")) is not int or receipt.get("format") != 1 or receipt.get("complete") is not True
            or receipt.get("source") != "steam-download-depot" or type(receipt.get("version")) is not int
            or receipt.get("app_id") != APP_ID or receipt.get("depot_id") != DEPOT_ID
            or receipt.get("version") != version or receipt.get("depot_manifest") != manifest):
        raise ValueError("Selected HQ game installation is incomplete or has changed")
    game = checked_path(base, directory / "game")
    program = checked_path(base, game / "Lethal Company.exe")
    data = checked_path(base, game / "Lethal Company_Data")
    unity = checked_path(base, game / "UnityPlayer.dll")
    prefix = checked_path(base, directory / "compatdata" / APP_ID)
    if not program.is_file() or not data.is_dir() or not unity.is_file():
        raise ValueError("Selected HQ game installation is missing required game files")
    positions = [i for i, value in enumerate(argv) if value.replace("\\", "/").rsplit("/", 1)[-1] == "Lethal Company.exe"]
    if len(positions) != 1:
        raise ValueError("Steam supplied an unsupported Lethal Company launch command")
    argv[positions[0]] = str(program)
    environ["STEAM_COMPAT_INSTALL_PATH"] = str(game)
    environ["STEAM_COMPAT_DATA_PATH"] = str(prefix)
    return argv, environ, str(game)


def main(argv):
    try:
        if not argv:
            raise ValueError("Steam did not supply a game launch command")
        command, environ, directory = prepare_launch(argv, os.environ)
        if directory is not None:
            os.makedirs(environ["STEAM_COMPAT_DATA_PATH"], exist_ok=True)
            os.chdir(directory)
            print("HQ: launching selected version from %s" % directory, file=sys.stderr, flush=True)
        os.execvpe(command[0], command, environ)
    except (OSError, ValueError) as error:
        print("HQ: cannot launch selected game version: %s" % error, file=sys.stderr, flush=True)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
