import assert from "node:assert/strict";
import { RUN_OPTIONS, RUN_MODE_VALUES } from "../src/pages/launcher/constants.js";

assert(!RUN_OPTIONS.some((option) => option.value === "lcf"), "LCF must be hidden from launcher settings");
assert(!RUN_MODE_VALUES.includes("lcf"), "Stored LCF preferences must fall back to HQ");
for (const mode of ["hq", "smhq", "eclipsed_hq", "practice", "diomedes"]) {
  assert(RUN_MODE_VALUES.includes(mode), `${mode} remains supported`);
}
for (const option of RUN_OPTIONS.filter((option) => option.value && !option.vanilla)) {
  assert(RUN_MODE_VALUES.includes(option.value), `${option.value} can be restored`);
}
console.log("LCF is hidden; supported launcher modes remain selectable.");
