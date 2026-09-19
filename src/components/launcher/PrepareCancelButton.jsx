import { invoke } from "@tauri-apps/api/core";
import { Button } from "../ui/button";

/**
 * Footer button for the practice/preset prepare modals: cancels an in-flight
 * prepare while it is running, and closes the modal once it is not.
 */
export function PrepareCancelButton({
  task,
  setTask,
  cancelBusy,
  setCancelBusy,
  onClose,
  onCancelStart,
  fallbackVersion,
}) {
  const status = task?.status ?? "working";
  return (
    <div className="mt-5 flex items-center justify-end gap-2">
      <Button
        variant="secondary"
        className="h-10 min-w-[120px]"
        disabled={cancelBusy}
        onClick={async () => {
          if ((task?.status ?? "working") !== "working") {
            onClose();
            return;
          }
          onCancelStart?.();
          const v = Number(task?.version ?? fallbackVersion);
          if (!Number.isFinite(v)) return;
          setCancelBusy(true);
          setTask((t) => ({
            ...(t ?? {}),
            status: "working",
            detail: "Cancelling...",
            error: null,
          }));
          try {
            await invoke("cancel_prepare", { version: v });
          } catch (e) {
            console.error(e);
            setCancelBusy(false);
          }
        }}
      >
        {status === "working"
          ? cancelBusy
            ? "Cancelling..."
            : "Cancel"
          : "Close"}
      </Button>
    </div>
  );
}
