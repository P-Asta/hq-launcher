import { invoke } from "@tauri-apps/api/core";
import { RUN_MODE_VALUES, SELECTED_EVENT_STORAGE_KEY } from "./constants";

export {
  getInitialEventsEnabled,
  saveEventsEnabled,
} from "../../lib/eventsSetting";

export function normalizeEventPreset(value) {
  const text = String(value ?? "hq").trim().toLowerCase();
  return RUN_MODE_VALUES.includes(text) ? text : "hq";
}

export function getInitialSelectedEventId() {
  if (typeof window === "undefined") return "";
  return localStorage.getItem(SELECTED_EVENT_STORAGE_KEY) ?? "";
}

export function saveSelectedEventId(eventId) {
  if (typeof window === "undefined") return;
  const normalized = String(eventId ?? "").trim();
  if (normalized) {
    localStorage.setItem(SELECTED_EVENT_STORAGE_KEY, normalized);
  } else {
    localStorage.removeItem(SELECTED_EVENT_STORAGE_KEY);
  }
}

export function parseUtcEventTime(value) {
  const text = String(value ?? "").trim();
  if (!text) return null;
  const parsed = Date.parse(text);
  return Number.isFinite(parsed) ? parsed : null;
}

export function formatEventTimeRemaining(event, nowMs = Date.now()) {
  const endsAt = parseUtcEventTime(event?.ends_at);
  if (endsAt === null) return "";

  const remainingMs = endsAt - nowMs;
  if (remainingMs <= 0) return "Ending now";

  if (remainingMs < 86_400_000) {
    const totalSeconds = Math.ceil(remainingMs / 1000);
    const hours = Math.floor(totalSeconds / 3600);
    const minutes = Math.floor((totalSeconds % 3600) / 60);
    const seconds = totalSeconds % 60;
    const secondText = String(seconds).padStart(2, "0");
    if (hours > 0) return `Ends in ${hours}h ${minutes}m ${secondText}s`;
    return `Ends in ${minutes}m ${secondText}s`;
  }

  const totalMinutes = Math.ceil(remainingMs / 60_000);
  if (totalMinutes <= 1) return "Ends soon";

  const days = Math.floor(totalMinutes / 1440);
  const hours = Math.floor((totalMinutes % 1440) / 60);
  const minutes = totalMinutes % 60;

  if (days > 0) {
    return `Ends in ${days}d${hours > 0 ? ` ${hours}h` : ""}`;
  }
  if (hours > 0) {
    return `Ends in ${hours}h${minutes > 0 ? ` ${minutes}m` : ""}`;
  }
  return `Ends in ${minutes}m`;
}

export function isEventActive(event, nowMs = Date.now(), testerAllowed = false) {
  const startsAt = parseUtcEventTime(event?.starts_at);
  if (!testerAllowed && startsAt !== null && nowMs < startsAt) return false;

  const endsAt = parseUtcEventTime(event?.ends_at);
  if (endsAt === null) return true;
  return nowMs <= endsAt;
}

export function normalizeSteamId(value) {
  const digits = String(value ?? "").replace(/\D/g, "");
  return digits.length === 17 && digits.startsWith("7656") ? digits : "";
}

export function eventAllowsTester(event, loginState) {
  const testerValues = Array.isArray(event?.testers) ? event.testers : [];
  const testerSteamIds = testerValues.map(normalizeSteamId).filter(Boolean);
  const testerNames = testerValues
    .filter((value) => !normalizeSteamId(value))
    .map((value) => String(value ?? "").trim().toLowerCase())
    .filter(Boolean);
  if (testerSteamIds.length === 0 && testerNames.length === 0) return true;

  const steamId = normalizeSteamId(loginState?.steam_id ?? loginState?.steamId);
  const username = String(loginState?.username ?? "").trim().toLowerCase();
  return (
    (!!steamId && testerSteamIds.includes(steamId)) ||
    (!!username && testerNames.includes(username))
  );
}

export function eventAllowsVersion(event, version) {
  const versions = Array.isArray(event?.versions)
    ? event.versions.map((v) => Number(v)).filter((v) => Number.isFinite(v))
    : [];
  if (versions.length === 0) return true;
  return versions.includes(Number(version));
}

export function clampVersionToEvent(version, event, fallbackVersions = []) {
  if (!event || eventAllowsVersion(event, version)) return Number(version);
  const allowed = Array.isArray(event.versions)
    ? event.versions.map((v) => Number(v)).filter((v) => Number.isFinite(v))
    : [];
  if (allowed.length === 0) return Number(version);
  const installedAllowed = allowed
    .filter((v) => fallbackVersions.includes(v))
    .sort((a, b) => b - a);
  return installedAllowed[0] ?? allowed.sort((a, b) => b - a)[0];
}

export function saveSelectedVersion(version) {
  if (typeof window === "undefined") return;
  const numeric = Number(version);
  if (Number.isFinite(numeric) && numeric > 0) {
    localStorage.setItem("selectedVersion", String(numeric));
    invoke("set_selected_version", { version: numeric }).catch(() => {});
  }
}
