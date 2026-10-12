import assert from "node:assert/strict";
import { acceptReplayWarning, hasAcceptedReplayWarning, needsReplayWarning } from "../src/pages/launcher/replay.js";

const stored = new Map();
globalThis.localStorage = {
  getItem: (key) => stored.get(key) ?? null,
  setItem: (key, value) => stored.set(key, value),
};
const replay = { dev: "asta", name: "Replay" };
assert.equal(hasAcceptedReplayWarning(), false);
assert.equal(needsReplayWarning([replay], true, hasAcceptedReplayWarning()), true);
assert.equal(needsReplayWarning([replay], false, false), false);
assert.equal(needsReplayWarning([{ dev: "another", name: "Replay" }], true, false), false);
assert.equal(needsReplayWarning([{ dev: "ASTA", name: "REPLAY" }], true, false), true);
acceptReplayWarning();
assert.equal(hasAcceptedReplayWarning(), true);
assert.equal(needsReplayWarning([replay], true, hasAcceptedReplayWarning()), false);
globalThis.localStorage = {
  getItem() { throw new Error("Storage unavailable"); },
  setItem() { throw new Error("Storage unavailable"); },
};
assert.equal(hasAcceptedReplayWarning(), false);
assert.doesNotThrow(acceptReplayWarning);
console.log("Replay warning checks passed.");
