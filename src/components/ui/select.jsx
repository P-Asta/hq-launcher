import * as React from "react";
import * as SelectPrimitive from "@radix-ui/react-select";
import { Check, ChevronDown } from "lucide-react";
import { cn } from "../../lib/cn";

export function Select({ value, onValueChange, children, disabled }) {
  return (
    <SelectPrimitive.Root value={value} onValueChange={onValueChange} disabled={disabled}>
      {children}
    </SelectPrimitive.Root>
  );
}

export function SelectTrigger({ className, children, showIcon = true, ...props }) {
  return (
    <SelectPrimitive.Trigger
      className={cn(
        "flex h-10 w-full items-center justify-between rounded-xl border border-white/10 bg-white/5 px-4 text-sm text-white outline-none transition-colors duration-150 focus:ring-2 focus:ring-white/10 disabled:opacity-50",
        className,
      )}
      {...props}
    >
      {children}
      {showIcon ? (
        <SelectPrimitive.Icon asChild>
          <ChevronDown className="h-4 w-4 text-white/50" />
        </SelectPrimitive.Icon>
      ) : null}
    </SelectPrimitive.Trigger>
  );
}

export function SelectValue({ placeholder }) {
  return <SelectPrimitive.Value placeholder={placeholder} />;
}

export function SelectContent({ className, children, ...props }) {
  return (
    <SelectPrimitive.Portal>
      <SelectPrimitive.Content
        position="popper"
        sideOffset={8}
        className={cn(
          "z-50 min-w-40 overflow-hidden rounded-2xl border border-white/10 bg-[#0f1116] shadow-2xl shadow-black/40",
          className,
        )}
        {...props}
      >
        <SelectPrimitive.Viewport className="p-1">{children}</SelectPrimitive.Viewport>
      </SelectPrimitive.Content>
    </SelectPrimitive.Portal>
  );
}

export function SelectItem({ className, value, children, marker }) {
  return (
    <SelectPrimitive.Item
      value={value}
      className={cn(
        "group relative flex cursor-pointer select-none items-center gap-2 rounded-xl px-3 py-2 text-sm text-white/85 outline-none focus:bg-white/10 data-disabled:pointer-events-none data-disabled:opacity-40",
        className,
      )}
    >
      <span className="absolute left-2 inline-flex w-5 items-center justify-center">
        <SelectPrimitive.ItemIndicator>
          <Check className="h-4 w-4 text-emerald-300" />
        </SelectPrimitive.ItemIndicator>
        {marker ? (
          <span className="group-data-[state=checked]:hidden">{marker}</span>
        ) : null}
      </span>
      <span className="pl-5">
        <SelectPrimitive.ItemText>{children}</SelectPrimitive.ItemText>
      </span>
    </SelectPrimitive.Item>
  );
}

