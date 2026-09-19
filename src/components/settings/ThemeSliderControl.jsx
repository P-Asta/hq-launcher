import { cn } from "../../lib/cn";

/**
 * Labelled number box plus a gradient range slider, both bound to one value.
 * `track` renders the gradient strip behind the slider.
 */
export function ThemeSliderControl({
  label,
  ariaLabel,
  value,
  min,
  max,
  onChange,
  track,
  valueClassName,
}) {
  const handleChange = (event) => onChange(event.target.value);
  return (
    <>
      <div className="flex items-center justify-between gap-3">
        <div className="text-sm font-semibold text-white">{label}</div>
        <input
          type="number"
          min={min}
          max={max}
          step="1"
          value={value}
          onChange={handleChange}
          className={cn(
            "h-8 w-16 rounded-md border border-panel-outline bg-black/20 px-2 text-center text-sm font-semibold outline-none focus:ring-2 focus:ring-panel-outline",
            valueClassName
          )}
          aria-label={`${ariaLabel} value`}
        />
      </div>

      <div className="relative h-7 rounded-md border border-white/10 p-1">
        {track}
        <input
          type="range"
          min={min}
          max={max}
          step="1"
          value={value}
          onChange={handleChange}
          className="theme-hue-slider absolute inset-0 h-full w-full cursor-pointer appearance-none bg-transparent"
          aria-label={ariaLabel}
        />
      </div>
    </>
  );
}
