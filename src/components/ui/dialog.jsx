import * as React from "react";
import * as DialogPrimitive from "@radix-ui/react-dialog";
import { cn } from "../../lib/cn";

export function Dialog({ open, onOpenChange, children }) {
  return (
    <DialogPrimitive.Root open={open} onOpenChange={onOpenChange}>
      {children}
    </DialogPrimitive.Root>
  );
}

export function DialogPortal({ children }) {
  return <DialogPrimitive.Portal>{children}</DialogPrimitive.Portal>;
}

export function DialogOverlay({ className, ...props }) {
  return (
    <DialogPrimitive.Overlay
      className={cn(
        "fixed inset-x-0 bottom-0 top-10 z-[60] bg-black/60 backdrop-blur-sm",
        className
      )}
      {...props}
    />
  );
}

export function DialogContent({ className, onEscapeKeyDown, onPointerDownOutside, children }) {
  return (
    <DialogPortal>
      <DialogOverlay />
      <DialogPrimitive.Content
        onEscapeKeyDown={onEscapeKeyDown}
        onPointerDownOutside={onPointerDownOutside}
        className={cn(
          "fixed left-1/2 top-[calc(50%+1.25rem)] z-[60] w-[min(560px,92vw)] -translate-x-1/2 -translate-y-1/2 rounded-3xl border border-panel-outline bg-[#0f1116] p-5 text-white shadow-2xl shadow-black/50",
          className
        )}
      >
        {children}
      </DialogPrimitive.Content>
    </DialogPortal>
  );
}

