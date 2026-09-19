/**
 * Decides whether an `updatable://*` event belongs to the check-update task
 * currently in state. Events for another version or run mode are stale and
 * must be ignored.
 *
 * @returns `{ stale, runMode }` — `runMode` is the run mode to carry forward.
 */
export function resolveUpdateEvent(task, payload) {
  const eventVersion = Number(payload?.version ?? task.version);
  const runMode =
    typeof payload?.run_mode === "string" && payload.run_mode
      ? payload.run_mode
      : task.run_mode;
  const taskVersion = Number(task.version);
  const stale =
    (Number.isFinite(taskVersion) && eventVersion !== taskVersion) ||
    Boolean(task.run_mode && runMode && runMode !== task.run_mode);
  return { stale, runMode };
}
