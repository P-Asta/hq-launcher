import { cn } from "../../lib/cn";
import {
  formatExtractProgress,
  formatTransferProgress,
} from "../../lib/format";

const pct = (value) => Math.max(0, Math.min(100, Number(value ?? 0)));

/** Thin accent progress bar; turns red when `error` is set. */
export function ProgressBar({ percent, error = false, className, trackClassName }) {
  return (
    <div
      className={cn(
        "h-2 w-full overflow-hidden rounded-full",
        trackClassName ?? "bg-white/10",
        className
      )}
    >
      <div
        className={cn(
          "h-full rounded-full transition-[width]",
          error ? "bg-red-400" : "bg-[var(--theme-accent)]"
        )}
        style={{ width: `${pct(percent)}%` }}
      />
    </div>
  );
}

/**
 * Boxed "title / detail / transfer+extract / error" row with a percentage on
 * the right and a progress bar underneath. `task` supplies the transfer and
 * extract sub-lines; pass `null` to omit them.
 */
export function TaskProgressPanel({
  title,
  detail,
  task,
  percentText,
  percent,
  error,
  barError = false,
  className,
}) {
  const transfer = task ? formatTransferProgress(task) : "";
  const extract = task ? formatExtractProgress(task) : "";
  return (
    <div
      className={cn(
        "mt-4 rounded-2xl border border-panel-outline bg-black/20 p-3",
        className
      )}
    >
      <div className="flex items-center justify-between gap-3">
        <div className="min-w-0">
          <div className="truncate text-sm font-semibold">{title}</div>
          <div className="truncate text-xs text-white/50">{detail}</div>
          {(transfer || extract) && (
            <div className="mt-1 text-xs text-white/40">
              {[transfer, extract].filter(Boolean).join(" • ")}
            </div>
          )}
          {error && <div className="mt-1 text-xs text-red-300">{error}</div>}
        </div>
        <div className="shrink-0 text-sm text-white/70">{percentText}</div>
      </div>
      <ProgressBar className="mt-2" percent={percent} error={barError} />
    </div>
  );
}
