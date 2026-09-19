import { cn } from "../../lib/cn";
import { LoaderCircle } from "lucide-react";

export function SkeletonBlock({ className }) {
  return (
    <div
      className={cn(
        "animate-pulse rounded-xl border border-panel-outline bg-white/[0.06]",
        className
      )}
    />
  );
}

export function LauncherPageSkeleton({ statusText }) {
  return (
    <>
      <div className="flex flex-wrap items-center gap-3">
        <SkeletonBlock className="h-11 w-[220px] rounded-xl" />
        <SkeletonBlock className="h-11 w-28 rounded-xl" />
        <SkeletonBlock className="h-11 min-w-[220px] flex-1 rounded-xl" />
        <SkeletonBlock className="h-11 w-11 rounded-xl" />
      </div>

      <div className="rounded-2xl border border-panel-outline bg-black/20 px-4 py-3">
        <div className="flex items-center gap-2 text-sm font-medium text-white/85">
          <LoaderCircle className="h-4 w-4 animate-spin text-white/65" />
          <span>Preparing launcher</span>
        </div>
        <div className="mt-1 text-xs text-white/50">
          {statusText || "Loading local versions and mod manifest..."}
        </div>
      </div>

      <div className="grid min-h-0 flex-1 grid-cols-1 gap-4">
        <div className="min-h-0 rounded-2xl border border-panel-outline bg-[var(--theme-surface)] p-3">
          <div className="mb-3 flex items-center justify-between px-1">
            <SkeletonBlock className="h-4 w-20 rounded-md" />
            <SkeletonBlock className="h-4 w-14 rounded-md" />
          </div>

          <div className="flex flex-col gap-2">
            {Array.from({ length: 6 }).map((_, index) => (
              <div
                key={index}
                className="flex items-start gap-3 rounded-2xl border border-panel-outline bg-black/10 px-3 py-3"
              >
                <SkeletonBlock className="h-11 w-11 shrink-0 rounded-xl" />
                <div className="min-w-0 flex-1 space-y-2">
                  <SkeletonBlock className="h-5 w-36 rounded-md" />
                  <SkeletonBlock className="h-4 w-24 rounded-md" />
                  <SkeletonBlock className="h-4 w-full rounded-md" />
                </div>
                <SkeletonBlock className="mt-2 h-6 w-10 shrink-0 rounded-full" />
              </div>
            ))}
          </div>
        </div>
      </div>
    </>
  );
}
