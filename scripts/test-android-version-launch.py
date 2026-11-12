"""Offline tests for the HQ-owned Steam launch wrapper."""
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest


sys.dont_write_bytecode = True
SCRIPT = Path(__file__).resolve().parents[1] / "android/runtime/src/main/assets/hq-lethal-launch.py"


class VersionLaunchTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="hq-version-launch-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.base = self.root / "hq-launcher"
        self.base.mkdir()
        self.argv = ["/runtime/steam-launch-wrapper", "--", "/tools/proton", "waitforexitandrun",
                     "/Steam/steamapps/common/Lethal Company/Lethal Company.exe", "-screen-fullscreen", "0"]
        self.env = {"SteamAppId": "1966720", "SteamGameId": "1966720", "SteamOverlayGameId": "1966720",
                    "STEAM_COMPAT_APP_ID": "1966720", "STEAM_COMPAT_DATA_PATH": "/Steam/steamapps/compatdata/1966720",
                    "STEAM_COMPAT_CLIENT_INSTALL_PATH": "/Steam", "WINEDLLOVERRIDES": "winhttp=n,b", "DISPLAY": ":1"}

    def implementation(self):
        self.assertTrue(SCRIPT.is_file(), "HQ version launch wrapper has not been implemented")
        spec = importlib.util.spec_from_file_location("hq_lethal_launch", SCRIPT)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def fixture(self):
        identity = {"format": 1, "version": 73, "depot_manifest": "1749099131234587692"}
        directory = self.base / "versions/73-1749099131234587692"
        game = directory / "game"
        game.mkdir(parents=True)
        (game / "Lethal Company.exe").write_bytes(b"MZfixture")
        (game / "Lethal Company_Data").mkdir()
        (game / "UnityPlayer.dll").write_bytes(b"MZfixture")
        receipt = dict(identity, source="steam-download-depot", app_id="1966720", depot_id="1966721", complete=True)
        (directory / "install.json").write_text(json.dumps(receipt), encoding="utf-8")
        (self.base / "selection.json").write_text(json.dumps(identity), encoding="utf-8")
        return directory, game, receipt

    def test_normal_steam_launch_passes_through_unchanged(self):
        self.assertEqual(self.implementation().prepare_launch(self.argv, self.env, self.base),
                         (self.argv, self.env, None))

    def test_selected_version_replaces_only_game_and_prefix(self):
        directory, game, _ = self.fixture()
        args, env, cwd = self.implementation().prepare_launch(self.argv, self.env, self.base)
        expected = list(self.argv)
        expected[4] = str(game / "Lethal Company.exe")
        self.assertEqual(args, expected)
        self.assertEqual(cwd, str(game))
        self.assertEqual(env, dict(self.env, STEAM_COMPAT_INSTALL_PATH=str(game),
                                  STEAM_COMPAT_DATA_PATH=str(directory / "compatdata/1966720")))
        self.assertEqual(self.argv[4], "/Steam/steamapps/common/Lethal Company/Lethal Company.exe")

    def test_incomplete_or_wrong_receipt_is_rejected(self):
        directory, _, receipt = self.fixture()
        implementation = self.implementation()
        for changes in ({"complete": False}, {"app_id": "480"}, {"depot_id": "1"}, {"version": 81},
                        {"depot_manifest": "other"}, {"format": 2}, {"format": True}, {"source": "manual-copy"}):
            with self.subTest(changes=changes):
                (directory / "install.json").write_text(json.dumps(dict(receipt, **changes)), encoding="utf-8")
                with self.assertRaises(ValueError):
                    implementation.prepare_launch(self.argv, self.env, self.base)

    def test_missing_files_and_duplicate_game_argument_fail_closed(self):
        directory, game, _ = self.fixture()
        implementation = self.implementation()
        with self.assertRaises(ValueError):
            implementation.prepare_launch(self.argv + [self.argv[4]], self.env, self.base)
        with self.assertRaises(ValueError):
            implementation.prepare_launch(self.argv[:4], self.env, self.base)
        (game / "Lethal Company.exe").unlink()
        with self.assertRaises(ValueError):
            implementation.prepare_launch(self.argv, self.env, self.base)

    def test_wrong_steam_app_identity_is_rejected(self):
        self.fixture()
        with self.assertRaises(ValueError):
            self.implementation().prepare_launch(self.argv, dict(self.env, SteamAppId="480"), self.base)

    def test_selected_installation_cannot_use_another_app_compat_id(self):
        self.fixture()
        with self.assertRaises(ValueError):
            self.implementation().prepare_launch(self.argv, dict(self.env, STEAM_COMPAT_APP_ID="0"), self.base)

    def test_invalid_selection_and_missing_unity_files_are_rejected(self):
        _, game, _ = self.fixture()
        implementation = self.implementation()
        (self.base / "selection.json").write_text("{bad json", encoding="utf-8")
        with self.assertRaises(ValueError):
            implementation.prepare_launch(self.argv, self.env, self.base)
        (self.base / "selection.json").write_text(json.dumps({"format": True, "version": 73,
                "depot_manifest": "1749099131234587692"}), encoding="utf-8")
        with self.assertRaises(ValueError):
            implementation.prepare_launch(self.argv, self.env, self.base)
        (self.base / "selection.json").write_text(json.dumps({"format": 1, "version": 73,
                "depot_manifest": "1749099131234587692"}), encoding="utf-8")
        (game / "UnityPlayer.dll").unlink()
        with self.assertRaises(ValueError):
            implementation.prepare_launch(self.argv, self.env, self.base)

    def test_selection_cannot_escape_the_version_store(self):
        self.fixture()
        implementation = self.implementation()
        for version, manifest in (("../73", "123"), (73, "../other"), (True, "123"), (73, "1\nquit")):
            with self.subTest(version=version, manifest=manifest):
                (self.base / "selection.json").write_text(json.dumps({"format": 1, "version": version,
                     "depot_manifest": manifest}), encoding="utf-8")
                with self.assertRaises(ValueError):
                    implementation.prepare_launch(self.argv, self.env, self.base)

    def test_linked_game_or_prefix_is_rejected(self):
        directory, game, _ = self.fixture()
        target = self.root / "outside"
        target.mkdir()
        prefix = directory / "compatdata"
        try:
            prefix.symlink_to(target, target_is_directory=True)
        except OSError as error:
            self.skipTest("Host cannot create symbolic links: " + str(error))
        with self.assertRaises(ValueError):
            self.implementation().prepare_launch(self.argv, self.env, self.base)


if __name__ == "__main__":
    unittest.main()
