import assert from "node:assert/strict";
import { getPresetVersionRange, getLaunchRequestForRunMode, getPresetSummarySpec, normalizeStoredRunMode, getInitialRunMode } from "../src/pages/launcher/runModes.js";

assert.deepEqual(getPresetVersionRange({ manifests: { 49: "a", 73: "b", 72: "c" } }, "diomedes"), { low: 73, high: 73 }, "Diomedes must clamp to latest catalog game version");
assert.deepEqual(getPresetVersionRange({ manifests: { 74: "new", 73: "old" } }, "diomedes"), { low: 74, high: 74 }, "latest follows catalog updates");
assert.equal(getPresetVersionRange({ manifests: {} }, "diomedes"), null);
assert.deepEqual(getLaunchRequestForRunMode("diomedes", 73), { command: "launch_game_preset", args: { version: 73, preset: "diomedes", practice: false } });
assert.equal(getPresetSummarySpec("diomedes")?.name, "Diomedes");
assert.equal(normalizeStoredRunMode("lcf"), "hq", "backend LCF preference falls back to HQ");
globalThis.localStorage = { getItem: () => "lcf" };
assert.equal(getInitialRunMode(), "hq", "local LCF preference falls back to HQ");
assert.equal(normalizeStoredRunMode("diomedes"), "diomedes");
console.log("Diomedes latest-game constraint, launch mapping and summary passed.");
