import assert from "node:assert/strict";
import * as mods from "../src/pages/launcher/mods.js";

const replay = { dev: "AsTa", name: "RePlAy", enabled: true, low_cap: 49 };
const cull = { dev: "fumiko", name: "CullFactory" };
assert.equal(typeof mods.isCullFactoryBlockedByReplay, "function", "Replay conflict rule must exist");
assert.equal(mods.isCullFactoryBlockedByReplay(cull, [replay, cull], new Set(), 73), true);
assert.equal(mods.isCullFactoryBlockedByReplay(cull, [replay, cull], new Set(["asta::replay"]), 73), false);
assert.equal(mods.isCullFactoryBlockedByReplay(cull, [cull], new Set(), 73), false);
assert.equal(mods.isCullFactoryBlockedByReplay(cull, [{ ...replay, enabled: false }], new Set(), 73), false);
assert.equal(mods.isCullFactoryBlockedByReplay(cull, [replay], new Set(), 40), false);
assert.equal(mods.isCullFactoryBlockedByReplay({ dev: "other", name: "CullFactory" }, [replay], new Set(), 73), false);
assert.equal(mods.isCullFactoryBlockedByReplay(replay, [replay], new Set(), 73), false);
console.log("Replay compatibility checks passed");
