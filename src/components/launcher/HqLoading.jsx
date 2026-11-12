import { cn } from "../../lib/cn";

/** The same HQ mark is used by web and native runtime loading surfaces. */
export function HqLogo({ className, active = true, progress }) {
  const percent = typeof progress === "number" && Number.isFinite(progress) && progress >= 0
    ? Math.min(100, progress) : null;
  return (
    <span data-hq-logo data-active={active} data-progress={percent ?? undefined}
      className={cn("hq-logo relative inline-block h-16 w-16 shrink-0", className)}
      style={percent === null ? undefined : { "--hq-logo-fill": `${100 - percent}%` }} aria-hidden="true">
      <img src="/icon.svg" alt="" draggable="false" className="hq-logo-base" />
      <img src="/icon.svg" alt="" draggable="false" className="hq-logo-fill" />
    </span>
  );
}

export function HqLoading({ title = "HQ Launcher", detail = "Starting up…", progress, screen = false, className }) {
  return (
    <div data-hq-loading={screen ? "screen" : "inline"} role="status" aria-live="polite"
      className={cn("hq-loading flex items-center gap-3", screen && "hq-loading-screen h-full min-h-40 w-full flex-col justify-center p-6 text-center text-white", className)}>
      <HqLogo progress={progress} className={screen ? "h-24 w-24" : "h-10 w-10"} />
      <div className="min-w-0">
        <div className={cn("font-semibold", screen ? "mt-4 text-lg" : "text-sm")}>{title}</div>
        {detail && <div className={cn("mt-1 break-words text-white/55", screen ? "text-sm" : "text-xs")}>{detail}</div>}
      </div>
    </div>
  );
}
