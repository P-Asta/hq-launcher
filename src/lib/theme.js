import { emit } from "@tauri-apps/api/event";

export const PRIMARY_COLOR_STORAGE_KEY = "hq-launcher-primary-color";
export const THEME_HUE_STORAGE_KEY = "hq-launcher-theme-hue";
export const THEME_BRIGHTNESS_STORAGE_KEY = "hq-launcher-theme-brightness";
export const THEME_MODE_STORAGE_KEY = "hq-launcher-theme-mode";
export const PRIMARY_COLOR_EVENT = "theme://primary-color-changed";
export const THEME_HUE_EVENT = "theme://hue-changed";
export const THEME_SETTINGS_EVENT = "theme://settings-changed";
export const DEFAULT_PRIMARY_COLOR = "#00c896";
export const DEFAULT_THEME_HUE = 160;
export const DEFAULT_THEME_BRIGHTNESS = 0;
export const DEFAULT_THEME_MODE = "dark";
export const THEME_MODES = ["dark", "light"];

function clampHue(value, fallback = DEFAULT_THEME_HUE) {
  const n = Number(value);
  if (!Number.isFinite(n)) return fallback;
  return Math.min(360, Math.max(0, Math.round(n)));
}

function clampBrightness(value, fallback = DEFAULT_THEME_BRIGHTNESS) {
  const n = Number(value);
  if (!Number.isFinite(n)) return fallback;
  return Math.min(100, Math.max(0, Math.round(n)));
}

function hexToHue(value) {
  if (typeof value !== "string" || !/^#[0-9a-fA-F]{6}$/.test(value.trim())) {
    return DEFAULT_THEME_HUE;
  }

  const hex = value.trim().slice(1);
  const r = parseInt(hex.slice(0, 2), 16) / 255;
  const g = parseInt(hex.slice(2, 4), 16) / 255;
  const b = parseInt(hex.slice(4, 6), 16) / 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const delta = max - min;

  if (delta === 0) return DEFAULT_THEME_HUE;

  let hue = 0;
  if (max === r) hue = 60 * (((g - b) / delta) % 6);
  if (max === g) hue = 60 * ((b - r) / delta + 2);
  if (max === b) hue = 60 * ((r - g) / delta + 4);
  return clampHue(hue < 0 ? hue + 360 : hue);
}

export function normalizeThemeHue(value, fallback = DEFAULT_THEME_HUE) {
  return clampHue(value, fallback);
}

export function normalizeThemeBrightness(value, fallback = DEFAULT_THEME_BRIGHTNESS) {
  return clampBrightness(value, fallback);
}

export function normalizeThemeMode(value, fallback = DEFAULT_THEME_MODE) {
  return THEME_MODES.includes(value) ? value : fallback;
}

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

export function loadStoredThemeHue() {
  if (typeof window === "undefined") return DEFAULT_THEME_HUE;
  try {
    const rawHue = window.localStorage.getItem(THEME_HUE_STORAGE_KEY);
    if (rawHue != null) return normalizeThemeHue(rawHue);

    const oldPrimary = window.localStorage.getItem(PRIMARY_COLOR_STORAGE_KEY);
    return hexToHue(oldPrimary ?? DEFAULT_PRIMARY_COLOR);
  } catch {
    return DEFAULT_THEME_HUE;
  }
}

export function loadStoredThemeBrightness() {
  if (typeof window === "undefined") return DEFAULT_THEME_BRIGHTNESS;
  try {
    const raw = window.localStorage.getItem(THEME_BRIGHTNESS_STORAGE_KEY);
    return normalizeThemeBrightness(raw);
  } catch {
    return DEFAULT_THEME_BRIGHTNESS;
  }
}

export function loadStoredThemeMode() {
  if (typeof window === "undefined") return DEFAULT_THEME_MODE;
  try {
    const raw = window.localStorage.getItem(THEME_MODE_STORAGE_KEY);
    return normalizeThemeMode(raw);
  } catch {
    return DEFAULT_THEME_MODE;
  }
}

export function saveThemeHue(hue) {
  const normalized = normalizeThemeHue(hue);
  window.localStorage.setItem(THEME_HUE_STORAGE_KEY, String(normalized));
  return normalized;
}

export function saveThemeBrightness(brightness) {
  const normalized = normalizeThemeBrightness(brightness);
  window.localStorage.setItem(THEME_BRIGHTNESS_STORAGE_KEY, String(normalized));
  return normalized;
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

export function saveThemeMode(mode) {
  const normalized = normalizeThemeMode(mode);
  window.localStorage.setItem(THEME_MODE_STORAGE_KEY, normalized);
  return normalized;
}

export function applyThemeSettings({ hue, brightness, mode } = {}) {
  const normalized = normalizeThemeHue(hue);
  const normalizedBrightness = normalizeThemeBrightness(brightness);
  const normalizedMode = normalizeThemeMode(mode);
  const root = document.documentElement;
  const lift = normalizedBrightness * 0.16;

  root.style.setProperty("--theme-hue", String(normalized));
  root.style.setProperty("--theme-brightness", String(normalizedBrightness));
  root.style.setProperty("--theme-mode", normalizedMode);
  root.classList.toggle("theme-light", normalizedMode === "light");
  root.classList.toggle("theme-dark", normalizedMode === "dark");

  if (normalizedMode === "light") {
    const lightLift = normalizedBrightness * 0.055;
    root.style.setProperty("--theme-accent", `hsl(${normalized} 82% 39%)`);
    root.style.setProperty("--theme-accent-strong", `hsl(${normalized} 86% 32%)`);
    root.style.setProperty("--theme-accent-muted", `hsl(${normalized} 78% ${89 - lightLift * 0.45}% / 0.82)`);
    root.style.setProperty("--theme-bg", `hsl(${normalized} 32% ${97.5 - lightLift}%)`);
    root.style.setProperty("--theme-surface", `hsl(${normalized} 26% ${99 - lightLift * 0.55}%)`);
    root.style.setProperty("--theme-panel", `hsl(${normalized} 30% ${95.5 - lightLift}%)`);
    root.style.setProperty("--theme-elevated", `hsl(${normalized} 34% ${98 - lightLift * 0.55}%)`);
    root.style.setProperty("--theme-border", `hsl(${normalized} 26% ${82 - lightLift * 0.75}%)`);
    root.style.setProperty("--theme-overlay", `hsl(${normalized} 28% ${98 - lightLift * 0.55}%)`);
    root.style.setProperty("--color-panel-outline", `hsl(${normalized} 26% ${82 - lightLift * 0.75}%)`);
    root.style.colorScheme = "light";
  } else {
    root.style.setProperty("--theme-accent", `hsl(${normalized} 82% 62%)`);
    root.style.setProperty("--theme-accent-strong", `hsl(${normalized} 84% 56%)`);
    root.style.setProperty("--theme-accent-muted", `hsl(${normalized} 44% 18% / 0.38)`);
    root.style.setProperty("--theme-bg", `hsl(${normalized} 18% ${4.5 + lift}%)`);
    root.style.setProperty("--theme-surface", `hsl(${normalized} 14% ${7 + lift}%)`);
    root.style.setProperty("--theme-panel", `hsl(${normalized} 13% ${8.5 + lift}%)`);
    root.style.setProperty("--theme-elevated", `hsl(${normalized} 12% ${10 + lift}%)`);
    root.style.setProperty("--theme-border", `hsl(${normalized} 10% ${17 + lift * 0.8}%)`);
    root.style.setProperty("--theme-overlay", `hsl(${normalized} 14% ${7 + lift}%)`);
    root.style.setProperty("--color-panel-outline", `hsl(${normalized} 10% ${17 + lift * 0.8}%)`);
    root.style.colorScheme = "dark";
  }

  return { hue: normalized, brightness: normalizedBrightness, mode: normalizedMode };
}

export function applyThemeHue(hue) {
  return applyThemeSettings({
    hue,
    brightness: loadStoredThemeBrightness(),
    mode: loadStoredThemeMode(),
  }).hue;
}

export async function persistAndBroadcastPrimaryColor(primaryColor) {
  const normalized = applyPrimaryColor(savePrimaryColor(primaryColor));
  await emit(PRIMARY_COLOR_EVENT, { primaryColor: normalized });
  return normalized;
}

export async function persistAndBroadcastThemeHue(hue) {
  const normalized = saveThemeHue(hue);
  const settings = applyThemeSettings({
    hue: normalized,
    brightness: loadStoredThemeBrightness(),
    mode: loadStoredThemeMode(),
  });
  await emit(THEME_HUE_EVENT, { hue: normalized });
  await emit(THEME_SETTINGS_EVENT, settings);
  return normalized;
}

export async function persistAndBroadcastThemeBrightness(brightness) {
  const normalized = saveThemeBrightness(brightness);
  const settings = applyThemeSettings({
    hue: loadStoredThemeHue(),
    brightness: normalized,
    mode: loadStoredThemeMode(),
  });
  await emit(THEME_SETTINGS_EVENT, settings);
  return normalized;
}

export async function persistAndBroadcastThemeMode(mode) {
  const normalized = saveThemeMode(mode);
  const settings = applyThemeSettings({
    hue: loadStoredThemeHue(),
    brightness: loadStoredThemeBrightness(),
    mode: normalized,
  });
  await emit(THEME_SETTINGS_EVENT, settings);
  return normalized;
}
