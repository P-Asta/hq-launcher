import { toOptionalNumber } from "../../lib/format";

export function modKey(mod) {
  return `${mod.dev}::${mod.name}`;
}

export function modKeyLower(mod) {
  return `${String(mod?.dev ?? "").toLowerCase()}::${String(
    mod?.name ?? ""
  ).toLowerCase()}`;
}

export function isUnityExplorerModName(name) {
  return String(name ?? "")
    .replace(/[^a-z0-9]/gi, "")
    .toLowerCase()
    .includes("unityexplorer");
}

export function configPathMatchesMod(path, mod) {
  const pathLower = String(path ?? "").toLowerCase();
  const devLower = String(mod?.dev ?? "").toLowerCase();
  const nameLower = String(mod?.name ?? "").toLowerCase();
  if (
    (devLower && pathLower.includes(devLower)) ||
    (nameLower && pathLower.includes(nameLower))
  ) {
    return true;
  }

  const fileName = pathLower.split("/").pop();
  return (
    isUnityExplorerModName(mod?.name) &&
    fileName === "com.sinai.unityexplorer.cfg"
  );
}

export function isLockedCfgEntry(configPath, sectionName, entryName) {
  return (
    String(configPath ?? "").toLowerCase() === "asta.evlog.cfg" &&
    String(sectionName ?? "") === "HQ Launcher" &&
    String(entryName ?? "") === "EventId"
  );
}

export function isUiHiddenMod(mod) {
  return Array.isArray(mod?.tags)
    ? mod.tags.some((tag) => String(tag).toLowerCase() === "ui_hidden")
    : false;
}

export function isModCompatibleWithVersion(mod, version) {
  const v = Number(version);
  if (!Number.isFinite(v)) return true;
  const lowCap = toOptionalNumber(mod?.low_cap);
  const highCap = toOptionalNumber(mod?.high_cap);
  if (lowCap != null && v < lowCap) return false;
  if (highCap != null && v > highCap) return false;
  return true;
}

export function isPresetSummaryMod(mod) {
  return mod?.isPresetSummary === true;
}

export function listEntryKey(entry) {
  return entry?.summary_id ?? modKey(entry);
}

export function matchesVersionCaps(version, lowCap, highCap) {
  const v = Number(version);
  if (!Number.isFinite(v)) return true;
  const low = toOptionalNumber(lowCap);
  const high = toOptionalNumber(highCap);
  if (low != null && v < low) return false;
  if (high != null && v > high) return false;
  return true;
}

export function findTagConstraint(mod, activeTag) {
  const constraints = mod?.tag_constraints;
  if (!constraints || typeof constraints !== "object") return null;
  for (const [tag, rule] of Object.entries(constraints)) {
    if (String(tag).toLowerCase() === String(activeTag).toLowerCase()) {
      return rule ?? null;
    }
  }
  return null;
}

export function modAppliesToTag(mod, activeTag) {
  const modTags = Array.isArray(mod?.tags) ? mod.tags : [];
  const matchesTag = modTags.some(
    (tag) => String(tag).toLowerCase() === String(activeTag).toLowerCase()
  );
  return matchesTag || findTagConstraint(mod, activeTag) != null;
}

export function modHasRunModeAffinity(mod) {
  const modTags = Array.isArray(mod?.tags) ? mod.tags : [];
  if (
    modTags.some((tag) => {
      const value = String(tag).toLowerCase();
      return (
        value === "brutal" ||
        value === "wesley" ||
        value === "smhq" ||
        value === "eclipsed" ||
        value === "c.moons"
      );
    })
  ) {
    return true;
  }

  const constraints = mod?.tag_constraints;
  if (!constraints || typeof constraints !== "object") return false;
  return Object.keys(constraints).some((tag) => {
    const value = String(tag).toLowerCase();
    return (
      value === "brutal" ||
      value === "wesley" ||
      value === "smhq" ||
      value === "eclipsed" ||
      value === "c.moons"
    );
  });
}

export function isModCompatibleWithTags(mod, version, activeTags) {
  for (const activeTag of Array.isArray(activeTags) ? activeTags : []) {
    if (!modAppliesToTag(mod, activeTag)) continue;

    const constraint = findTagConstraint(mod, activeTag);
    const lowCap = constraint?.low_cap ?? mod?.low_cap ?? null;
    const highCap = constraint?.high_cap ?? mod?.high_cap ?? null;
    if (matchesVersionCaps(version, lowCap, highCap)) {
      return true;
    }
  }

  return false;
}

/**
 * Mods from `mods` whose key is in `keys`, that are enabled, not UI-hidden and
 * compatible with `version`.
 */
export function filterForcedMods(mods, keys, version) {
  const list = Array.isArray(mods) ? mods : [];
  return list.filter(
    (m) =>
      keys.has(modKeyLower(m)) &&
      m?.enabled !== false &&
      !isUiHiddenMod(m) &&
      isModCompatibleWithVersion(m, version)
  );
}
