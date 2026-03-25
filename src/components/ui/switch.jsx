import * as React from "react";
import { cn } from "../../lib/cn";

export function Switch({
  className,
  checked,
  onCheckedChange,
  disabled,
  onClick,
  ...props
}) {
  const state = checked ? "checked" : "unchecked";
  return (
    <button
      type="button"
      role="switch"
      aria-checked={!!checked}
      disabled={disabled}
      data-state={state}
      onClick={(e) => {
        onClick?.(e);
        if (disabled) return;
        onCheckedChange?.(!checked, e);
      }}
      className={cn(
        "relative inline-flex h-6 w-11 shrink-0 cursor-pointer items-center rounded-full border border-panel-outline bg-white/10 transition-colors",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-panel-outline",
        "disabled:cursor-not-allowed disabled:opacity-50",
        "data-[state=checked]:border-emerald-400 data-[state=checked]:bg-emerald-400",
        className
      )}
      {...props}
    >
      <span
        data-state={state}
        className={cn(
          "pointer-events-none inline-block h-5 w-5 translate-x-0.5 rounded-full bg-white shadow-sm transition-transform",
          "data-[state=checked]:translate-x-5 data-[state=checked]:bg-black"
        )}
      />
    </button>
  );
}

