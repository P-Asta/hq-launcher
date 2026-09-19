import { LAUNCH_OPTIONS_STORAGE_KEY } from "./constants";

export function makeDeleteVersionPromptState(overrides = {}) {
  return {
    open: false,
    version: null,
    error: "",
    status: "idle",
    overall_percent: 0,
    detail: "",
    deleted_files: 0,
    total_files: 0,
    ...overrides,
  };
}

export function normalizeLaunchOptionsEntries(entries) {
  if (!Array.isArray(entries)) return [];
  return entries
    .map((entry) => String(entry ?? "").trim())
    .filter(Boolean);
}

export function normalizeLaunchCommandTemplate(value) {
  return String(value ?? "").trim();
}

export function getInitialLaunchOptionsConfig() {
  if (typeof window === "undefined") {
    return { enabled: false, entries: [], commandTemplate: "" };
  }

  try {
    const raw = localStorage.getItem(LAUNCH_OPTIONS_STORAGE_KEY);
    if (!raw) return { enabled: false, entries: [], commandTemplate: "" };
    const parsed = JSON.parse(raw);
    return {
      enabled: !!parsed?.enabled,
      entries: normalizeLaunchOptionsEntries(parsed?.entries),
      commandTemplate: normalizeLaunchCommandTemplate(parsed?.commandTemplate),
    };
  } catch {
    return { enabled: false, entries: [], commandTemplate: "" };
  }
}
