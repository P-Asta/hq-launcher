const REPLAY_WARNING_STORAGE_KEY = "launcherReplayPerformanceWarningAccepted";

export function hasAcceptedReplayWarning() {
  try {
    return localStorage.getItem(REPLAY_WARNING_STORAGE_KEY) === "true";
  } catch {
    return false;
  }
}

export function acceptReplayWarning() {
  try {
    localStorage.setItem(REPLAY_WARNING_STORAGE_KEY, "true");
  } catch {
    // The current session still remembers consent if storage is unavailable.
  }
}

export function needsReplayWarning(mods, enabled, accepted) {
  return !!enabled && !accepted && mods.some((mod) =>
    String(mod?.dev).toLowerCase() === "asta" &&
    String(mod?.name).toLowerCase() === "replay"
  );
}
