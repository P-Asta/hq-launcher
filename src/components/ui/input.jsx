import { cn } from "../../lib/cn";

export function Input({ className, ...props }) {
  return (
    <input
      className={cn(
        "h-10 w-full rounded-xl border border-panel-outline bg-white/5 px-4 text-sm text-white placeholder:text-white/40 outline-none focus:border-panel-outline focus:ring-2 focus:ring-panel-outline",
        className,
      )}
      {...props}
    />
  );
}

