import { cn } from "../../lib/cn";
import { isInteractiveDragTarget, safeClassName } from "../../overlay/layout";

export function OverlayModuleView({ entry, position, editMode, onDragStart }) {
  const { module, widgetId, html } = entry;
  const scopedClass = `overlay-module overlay-module-${safeClassName(module.id)}`;
  return (
    <div
      data-overlay-widget={widgetId}
      className={cn(
        "fixed z-[2147483000] text-white",
        scopedClass,
        module.wrapperClass,
        editMode && !module.locked
          ? "pointer-events-auto cursor-move ring-1 ring-[var(--theme-accent)]/45"
          : "pointer-events-none",
      )}
      style={{ left: `${position.x}%`, top: `${position.y}%` }}
      onPointerDown={(event) => {
        if (!editMode || module.locked || isInteractiveDragTarget(event.target)) return;
        onDragStart(event, widgetId, module.id);
      }}
    >
      <div dangerouslySetInnerHTML={{ __html: html }} />
    </div>
  );
}
