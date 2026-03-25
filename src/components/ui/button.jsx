import { cva } from "class-variance-authority";
import { cn } from "../../lib/cn";

const buttonVariants = cva(
  "cursor-pointer inline-flex items-center justify-center gap-2 rounded-xl px-4 py-2 text-[14px] font-medium tracking-[-0.012em] transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-panel-outline disabled:opacity-50 disabled:pointer-events-none",
  {
    variants: {
      variant: {
        default: "bg-white text-black hover:bg-white/90",
        secondary:
          "bg-white/10 text-white hover:bg-white/15 border border-panel-outline",
        outline:
          "bg-transparent text-white hover:bg-white/10 border border-panel-outline",
        ghost: "bg-transparent text-white hover:bg-white/10",
      },
      size: {
        sm: "h-9 px-3",
        md: "h-10 px-4",
      },
    },
    defaultVariants: {
      variant: "secondary",
      size: "md",
    },
  }
);

export function Button({ className, variant, size, ...props }) {
  return (
    <button
      className={cn(buttonVariants({ variant, size }), className)}
      {...props}
    />
  );
}
