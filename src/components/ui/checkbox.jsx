import * as React from "react";
import * as CheckboxPrimitive from "@radix-ui/react-checkbox";
import { Check } from "lucide-react";
import { cn } from "../../lib/cn";

export function Checkbox({ className, checked, onCheckedChange, disabled }) {
  return (
    <CheckboxPrimitive.Root
      className={cn(
        "flex h-5 w-5 items-center justify-center rounded-md border border-white/20 bg-white/5 text-white shadow-sm outline-none transition focus:ring-2 focus:ring-white/10 data-[state=checked]:bg-emerald-400 data-[state=checked]:text-black data-[disabled]:opacity-50",
        className,
      )}
      checked={checked}
      onCheckedChange={onCheckedChange}
      disabled={disabled}
    >
      <CheckboxPrimitive.Indicator>
        <Check className="h-4 w-4" />
      </CheckboxPrimitive.Indicator>
    </CheckboxPrimitive.Root>
  );
}

