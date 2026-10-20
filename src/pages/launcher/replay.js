const REPLAY_WARNING_STORAGE_KEY = "launcherReplayPerformanceWarningAccepted";

export const REPLAY_STORAGE_WARNING =
  "Replay recordings can use a lot of disk space: a single in-game day can reach around 500 MB. Check free disk space and delete old replays regularly. CullFactory is automatically disabled while Replay is enabled.";

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
