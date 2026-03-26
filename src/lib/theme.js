import { emit } from "@tauri-apps/api/event";

export const PRIMARY_COLOR_STORAGE_KEY = "hq-launcher-primary-color";
export const PRIMARY_COLOR_EVENT = "theme://primary-color-changed";
export const DEFAULT_PRIMARY_COLOR = "#00c896";

export function normalizePrimaryColor(value, fallback = DEFAULT_PRIMARY_COLOR) {
  if (typeof value !== "string") return fallback;
  const trimmed = value.trim();
  if (/^#[0-9a-fA-F]{6}$/.test(trimmed)) return trimmed.toLowerCase();
  return fallback;
}

export function loadStoredPrimaryColor() {
  if (typeof window === "undefined") return DEFAULT_PRIMARY_COLOR;
  try {
    const raw = window.localStorage.getItem(PRIMARY_COLOR_STORAGE_KEY);
    return normalizePrimaryColor(raw, DEFAULT_PRIMARY_COLOR);
  } catch {
    return DEFAULT_PRIMARY_COLOR;
  }
}

export function savePrimaryColor(primaryColor) {
  const normalized = normalizePrimaryColor(primaryColor);
  window.localStorage.setItem(PRIMARY_COLOR_STORAGE_KEY, normalized);
  return normalized;
}

export function applyPrimaryColor(primaryColor) {
  const normalized = normalizePrimaryColor(primaryColor);
  document.documentElement.style.setProperty("--theme-accent", normalized);
  return normalized;
}

export async function persistAndBroadcastPrimaryColor(primaryColor) {
  const normalized = applyPrimaryColor(savePrimaryColor(primaryColor));
  await emit(PRIMARY_COLOR_EVENT, { primaryColor: normalized });
  return normalized;
}
