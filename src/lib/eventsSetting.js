import { invoke } from "@tauri-apps/api/core";

/** localStorage key mirroring the backend's "events enabled" flag. */
export const EVENTS_ENABLED_STORAGE_KEY = "launcherEventsEnabled";

/** Stored flag, or `null` when the user has never set one. */
export function readStoredEventsEnabled() {
  if (typeof window === "undefined") return null;
  return localStorage.getItem(EVENTS_ENABLED_STORAGE_KEY);
}

/** Initial value for local state: stored flag, defaulting to enabled. */
export function getInitialEventsEnabled() {
  const stored = readStoredEventsEnabled();
  return stored == null ? true : stored === "true";
}

/** Writes the flag to localStorage only. */
export function storeEventsEnabled(enabled) {
  if (typeof window === "undefined") return;
  localStorage.setItem(EVENTS_ENABLED_STORAGE_KEY, enabled ? "true" : "false");
}

/** Writes the flag to localStorage and pushes it to the backend. */
export function saveEventsEnabled(enabled) {
  if (typeof window === "undefined") return;
  storeEventsEnabled(enabled);
  invoke("set_events_enabled", { enabled: !!enabled }).catch(() => {});
}

/**
 * Reconciles the backend flag with the stored one on startup, pushing the
 * stored value back when they disagree. Returns the value to display.
 */
export function reconcileEventsEnabled(backendEnabled) {
  const stored = readStoredEventsEnabled();
  const next = stored == null ? !!backendEnabled : stored === "true";
  storeEventsEnabled(next);
  if (next !== !!backendEnabled) {
    invoke("set_events_enabled", { enabled: next }).catch(() => {});
  }
  return next;
}
