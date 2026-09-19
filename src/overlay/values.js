export function formatSeconds(totalSeconds) {
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}

export function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

export function number(value) {
  const numeric = Number(value ?? 0);
  if (!Number.isFinite(numeric)) return "0";
  return numeric.toLocaleString();
}

export function stripLcQuote(value) {
  return typeof value === "string" ? value.trim().replace(/^'/, "") : value;
}

export function intish(value, fallback = 0) {
  const cleaned = stripLcQuote(value);
  const numeric = Number(cleaned ?? fallback);
  return Number.isFinite(numeric) ? numeric : fallback;
}

export function valueAt(root, path, fallback = undefined) {
  const parts = Array.isArray(path) ? path : String(path).split(".");
  let current = root;
  for (const part of parts) {
    if (current == null || typeof current !== "object" || !(part in current)) {
      return fallback;
    }
    current = current[part];
  }
  return current ?? fallback;
}

export function valueAtAny(root, paths, fallback = undefined) {
  for (const path of paths) {
    const value = valueAt(root, path);
    if (value !== undefined && value !== null) return value;
  }
  return fallback;
}

export function firstValue(...values) {
  for (const value of values) {
    if (value !== undefined && value !== null && value !== "") return value;
  }
  return undefined;
}

export function arrayValue(value) {
  return Array.isArray(value) ? value : [];
}

export function pickMatchingKey(object, pattern) {
  if (!object || typeof object !== "object") return undefined;
  for (const [key, value] of Object.entries(object)) {
    if (pattern.test(key)) return value;
  }
  return undefined;
}

export function normalizeVersion(value) {
  const text = String(value ?? "").trim();
  if (!text) return "";
  return text.toLowerCase().startsWith("v") ? text.toLowerCase() : `v${text}`.toLowerCase();
}
