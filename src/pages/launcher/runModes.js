import { invoke } from "@tauri-apps/api/core";
import { toOptionalNumber } from "../../lib/format";
import { RUN_MODE_VALUES } from "./constants";

export function isPracticeRunMode(mode) {
  return String(mode ?? "").toLowerCase().includes("practice");
}

export function isSmhqRunMode(mode) {
  return String(mode ?? "").toLowerCase().includes("smhq");
}

export function isEclipsedRunMode(mode) {
  return String(mode ?? "").toLowerCase().includes("eclipsed");
}

export function isEclipsedHqRunMode(mode) {
  return String(mode ?? "").toLowerCase() === "eclipsed_hq";
}

export function shouldResetFreeMoonsOnRunModeChange(mode) {
  return isSmhqRunMode(mode) || isEclipsedHqRunMode(mode);
}

export function getRunModePresetTags(mode) {
  if (mode === "brutal" || mode === "brutal_practice") return ["Brutal"];
  if (mode === "brutal_smhq") return ["Brutal", "SMHQ"];
  if (mode === "brutal_eclipsed") return ["Brutal", "Eclipsed"];
  if (mode === "c_moons" || mode === "c_moons_practice" || mode === "c_moons_smhq")
    return ["C.Moons"];
  if (mode === "c_moons_eclipsed") return ["C.Moons", "Eclipsed"];
  if (mode === "wesley" || mode === "wesley_practice" || mode === "wesley_smhq")
    return ["Wesley"];
  if (mode === "wesley_eclipsed") return ["Wesley", "Eclipsed"];
  if (mode === "eclipsed_hq") return ["Eclipsed"];
  return [];
}

export function getManifestPresetConstraint(manifest, tag) {
  const entries = Object.entries(manifest?.preset_tag_constraints ?? {});
  const match = entries.find(([key]) => String(key).toLowerCase() === String(tag).toLowerCase());
  return match?.[1] ?? null;
}

export function getPresetVersionRange(manifest, mode) {
  const tags = getRunModePresetTags(mode);
  let low = null;
  let high = null;

  for (const tag of tags) {
    const rule = getManifestPresetConstraint(manifest, tag);
    if (!rule) continue;
    const lowCap = toOptionalNumber(rule?.low_cap);
    const highCap = toOptionalNumber(rule?.high_cap);
    if (lowCap != null) low = low == null ? lowCap : Math.max(low, lowCap);
    if (highCap != null) high = high == null ? highCap : Math.min(high, highCap);
  }

  if (low == null && high == null) return null;
  return { low, high };
}

export function isVersionWithinRange(version, range) {
  const v = Number(version);
  if (!Number.isFinite(v) || !range) return true;
  if (range.low != null && v < range.low) return false;
  if (range.high != null && v > range.high) return false;
  return true;
}

export function clampVersionToRange(version, range) {
  const v = Number(version);
  if (!Number.isFinite(v) || !range) return v;
  if (range.low != null && v < range.low) return range.low;
  if (range.high != null && v > range.high) return range.high;
  return v;
}

export function getInitialRunMode() {
  const savedRunMode = localStorage.getItem("selectedRunMode");
  return RUN_MODE_VALUES.includes(savedRunMode) ? savedRunMode : "hq";
}

export function saveSelectedRunMode(mode) {
  if (typeof window === "undefined") return;
  if (RUN_MODE_VALUES.includes(mode)) {
    localStorage.setItem("selectedRunMode", mode);
    invoke("set_selected_run_mode", { runMode: mode }).catch(() => {});
  }
}

export function getLaunchRequestForRunMode(mode, version) {
  if (mode === "vanilla") {
    return {
      command: "launch_game_vanilla",
      args: { version },
    };
  }
  if (mode === "practice") {
    return {
      command: "launch_game_practice",
      args: { version },
    };
  }
  if (mode === "brutal") {
    return {
      command: "launch_game_preset",
      args: { version, preset: "brutal", practice: false },
    };
  }
  if (mode === "brutal_practice") {
    return {
      command: "launch_game_preset",
      args: { version, preset: "brutal", practice: true },
    };
  }
  if (mode === "brutal_smhq") {
    return {
      command: "launch_game_preset",
      args: { version, preset: "brutal_smhq", practice: false },
    };
  }
  if (mode === "brutal_eclipsed") {
    return {
      command: "launch_game_preset",
      args: { version, preset: "brutal_eclipsed", practice: false },
    };
  }
  if (mode === "wesley") {
    return {
      command: "launch_game_preset",
      args: { version, preset: "wesley", practice: false },
    };
  }
  if (mode === "wesley_practice") {
    return {
      command: "launch_game_preset",
      args: { version, preset: "wesley", practice: true },
    };
  }
  if (mode === "wesley_smhq") {
    return {
      command: "launch_game_preset",
      args: { version, preset: "wesley_smhq", practice: false },
    };
  }
  if (mode === "wesley_eclipsed") {
    return {
      command: "launch_game_preset",
      args: { version, preset: "wesley_eclipsed", practice: false },
    };
  }
  if (mode === "smhq") {
    return {
      command: "launch_game_preset",
      args: { version, preset: "smhq", practice: false },
    };
  }
  if (mode === "c_moons") {
    return {
      command: "launch_game_preset",
      args: { version, preset: "c_moons", practice: false },
    };
  }
  if (mode === "c_moons_practice") {
    return {
      command: "launch_game_preset",
      args: { version, preset: "c_moons", practice: true },
    };
  }
  if (mode === "c_moons_smhq") {
    return {
      command: "launch_game_preset",
      args: { version, preset: "c_moons_smhq", practice: false },
    };
  }
  if (mode === "c_moons_eclipsed") {
    return {
      command: "launch_game_preset",
      args: { version, preset: "c_moons_eclipsed", practice: false },
    };
  }
  if (mode === "eclipsed_hq") {
    return {
      command: "launch_game_preset",
      args: { version, preset: "eclipsed_hq", practice: false },
    };
  }
  return {
    command: "launch_game",
    args: { version },
  };
}

export function getPresetSummarySpec(mode) {
  if (mode === "brutal" || mode === "brutal_practice") {
    return {
      summary_id: "preset::brutal",
      name: "Brutal Mods",
      subtitle: "",
      activeTags: ["Brutal"],
      iconKey: "drinkablewater::brutal_company_minus",
    };
  }

  if (mode === "brutal_smhq") {
    return {
      summary_id: "preset::brutal_smhq",
      name: "Brutal Mods",
      subtitle: "",
      activeTags: ["Brutal", "SMHQ"],
      iconKey: "drinkablewater::brutal_company_minus",
    };
  }

  if (mode === "brutal_eclipsed") {
    return {
      summary_id: "preset::brutal_eclipsed",
      name: "Brutal Mods",
      subtitle: "",
      activeTags: ["Brutal", "Eclipsed"],
      iconKey: "drinkablewater::brutal_company_minus",
    };
  }

  if (mode === "wesley" || mode === "wesley_practice") {
    return {
      summary_id: "preset::wesley",
      name: "Wesley's Mods",
      subtitle: "",
      activeTags: ["Wesley"],
      iconKey: "magic_wesley::wesleys_moons",
    };
  }

  if (mode === "wesley_smhq") {
    return {
      summary_id: "preset::wesley_smhq",
      name: "Wesley's Mods",
      subtitle: "",
      activeTags: ["Wesley", "SMHQ"],
      iconKey: "magic_wesley::wesleys_moons",
    };
  }

  if (mode === "wesley_eclipsed") {
    return {
      summary_id: "preset::wesley_eclipsed",
      name: "Wesley's Mods",
      subtitle: "",
      activeTags: ["Wesley", "Eclipsed"],
      iconKey: "magic_wesley::wesleys_moons",
    };
  }

  if (mode === "c_moons" || mode === "c_moons_practice") {
    return {
      summary_id: "preset::c_moons",
      name: "C.Moons Mods",
      subtitle: "",
      activeTags: ["C.Moons"],
      iconKey: "willowpillows::5_tandraus",
    };
  }

  if (mode === "c_moons_smhq") {
    return {
      summary_id: "preset::c_moons_smhq",
      name: "C.Moons Mods",
      subtitle: "",
      activeTags: ["C.Moons", "SMHQ"],
      iconKey: "willowpillows::5_tandraus",
    };
  }

  if (mode === "c_moons_eclipsed") {
    return {
      summary_id: "preset::c_moons_eclipsed",
      name: "C.Moons Mods",
      subtitle: "",
      activeTags: ["C.Moons", "Eclipsed"],
      iconKey: "willowpillows::5_tandraus",
    };
  }

  return null;
}

export function getPresetModulePriority(mod, activeTags) {
  if (String(mod?.dev ?? "").toLowerCase() !== "tomatobird") return 1;
  const name = String(mod?.name ?? "").toLowerCase();
  const tags = Array.isArray(activeTags) ? activeTags : [];

  if (tags.includes("Brutal") && name === "bcmhqmodule") return 0;
  if (tags.includes("Wesley") && name === "wesleysmoonshqmodule") return 0;
  if (tags.includes("C.Moons") && name === "classicmoonshqmodule") return 0;

  return 1;
}
