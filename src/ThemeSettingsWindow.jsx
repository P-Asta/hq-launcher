import { useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Paintbrush, RotateCcw } from "lucide-react";
import { Button } from "./components/ui/button";
import { Input } from "./components/ui/input";
import {
  applyPrimaryColor,
  DEFAULT_PRIMARY_COLOR,
  loadStoredPrimaryColor,
  normalizePrimaryColor,
  persistAndBroadcastPrimaryColor,
  PRIMARY_COLOR_EVENT,
} from "./lib/theme";

export default function ThemeSettingsWindow() {
  const [primaryColor, setPrimaryColor] = useState(() => loadStoredPrimaryColor());
  const normalizedPrimaryColor = useMemo(
    () => normalizePrimaryColor(primaryColor),
    [primaryColor]
  );

  useEffect(() => {
    applyPrimaryColor(normalizedPrimaryColor);
  }, [normalizedPrimaryColor]);

  useEffect(() => {
    let unlisten = null;

    (async () => {
      unlisten = await listen(PRIMARY_COLOR_EVENT, (event) => {
        const nextColor = normalizePrimaryColor(event.payload?.primaryColor);
        setPrimaryColor(nextColor);
        applyPrimaryColor(nextColor);
      });
    })();

    return () => {
      if (typeof unlisten === "function") unlisten();
    };
  }, []);

  async function handleApply(nextColor) {
    const applied = await persistAndBroadcastPrimaryColor(nextColor);
    setPrimaryColor(applied);
  }

  return (
    <div className="min-h-screen bg-[var(--theme-bg)] text-white">
      <div className="mx-auto flex min-h-screen w-full max-w-md flex-col px-6 py-8">
        <div className="flex items-start justify-between gap-4">
          <div>
            <div className="flex items-center gap-2 text-lg font-semibold">
              <Paintbrush className="h-5 w-5" />
              <span>Theme Settings</span>
            </div>
            <div className="mt-1 text-sm text-white/55">
              Only the primary color is configurable for now.
            </div>
          </div>
          <Button
            variant="outline"
            className="h-10 shrink-0"
            onClick={() => {
              void handleApply(DEFAULT_PRIMARY_COLOR);
            }}
          >
            <RotateCcw className="h-4 w-4" />
            Reset
          </Button>
        </div>

        <div className="mt-6 text-xs font-semibold uppercase tracking-[0.14em] text-white/45">
          Primary Color
        </div>
        <div
          className="mt-3 flex items-center gap-3 rounded-2xl border border-panel-outline p-3"
          style={{ backgroundColor: "var(--theme-overlay)" }}
        >
          <label
            className="flex h-12 w-12 shrink-0 cursor-pointer items-center justify-center rounded-xl border border-white/10 p-1"
            style={{ backgroundColor: "color-mix(in srgb, var(--theme-accent) 18%, transparent)" }}
          >
            <input
              type="color"
              className="h-full w-full cursor-pointer rounded-lg border-0 bg-transparent p-0"
              value={normalizedPrimaryColor}
              onChange={(event) => {
                const nextColor = normalizePrimaryColor(event.target.value);
                setPrimaryColor(nextColor);
                void handleApply(nextColor);
              }}
            />
          </label>

          <div className="min-w-0 flex-1">
            <div className="mb-1 text-[11px] font-medium uppercase tracking-[0.12em] text-white/35">
              Hex
            </div>
            <Input
              className="h-11 bg-transparent px-0 text-lg font-semibold tracking-[-0.02em] shadow-none"
              style={{ backgroundColor: "transparent" }}
              value={primaryColor}
              onChange={(event) => setPrimaryColor(event.target.value)}
              onBlur={() => {
                void handleApply(primaryColor);
              }}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  void handleApply(primaryColor);
                }
              }}
            />
          </div>
        </div>
        <div className="mt-2 pl-1 text-xs text-white/45">
          Applied immediately to the launcher buttons.
        </div>
      </div>
    </div>
  );
}
