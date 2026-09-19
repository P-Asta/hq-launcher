import { cn } from "../../lib/cn";

/**
 * Bordered settings row: optional leading icon, a title, an optional
 * description, and the control (children) pinned to the right.
 */
export function SettingRow({ icon, title, description, children, footer, className }) {
  const text = (
    <div className="min-w-0">
      <div className="text-sm font-semibold text-white">{title}</div>
      {description ? (
        <div className="mt-1 text-sm leading-5 text-white/55">{description}</div>
      ) : null}
    </div>
  );
  return (
    <div className={cn("rounded-lg border border-panel-outline p-4", className)}>
      <div className="flex items-start justify-between gap-4">
        {icon ? (
          <div className="flex min-w-0 gap-3">
            <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-md border border-white/10 bg-black/20 text-white/75">
              {icon}
            </div>
            {text}
          </div>
        ) : (
          text
        )}
        {children}
      </div>
      {footer}
    </div>
  );
}
