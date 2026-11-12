import { cn } from "../../lib/cn";
import { HqLoading } from "./HqLoading";

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
  return <HqLoading screen className="flex-1" title="Preparing launcher"
    detail={statusText || "Loading local versions and mod manifest..."} />;
}
