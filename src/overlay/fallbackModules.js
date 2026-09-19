import { escapeHtml, formatSeconds } from "./values";

export const fallbackCrosshairState = {
  runtimeEnabled: null,
  lastSettingEnabled: null,
};

export function isFallbackCrosshairEnabled(settings, api) {
  const settingEnabled = settings.enabled !== false;
  if (fallbackCrosshairState.runtimeEnabled == null || fallbackCrosshairState.lastSettingEnabled !== settingEnabled) {
    fallbackCrosshairState.runtimeEnabled = settingEnabled;
    fallbackCrosshairState.lastSettingEnabled = settingEnabled;
  }
  if (settings.toggleKey && api?.input?.consumePress(settings.toggleKey)) {
    fallbackCrosshairState.runtimeEnabled = !fallbackCrosshairState.runtimeEnabled;
  }
  return fallbackCrosshairState.runtimeEnabled;
}

export const FALLBACK_MODULES = [
  {
    id: "crosshair",
    fileName: "builtin:fallback-crosshair",
    name: "Crosshair",
    description: "Built-in fallback shown when overlay modules cannot be loaded.",
    locked: true,
    defaultPosition: { x: 50, y: 50 },
    defaultSettings: {
      enabled: false,
      toggleKey: "",
      style: "plus",
      color: "#ffffff",
      size: 24,
      thickness: 2,
      gap: 5,
      opacity: 0.9,
    },
    settings: [
      { key: "enabled", label: "Enabled", type: "boolean", default: false },
      { key: "toggleKey", label: "Toggle Key", type: "key", default: "" },
      {
        key: "style",
        label: "Style",
        type: "select",
        options: [
          { label: "Plus", value: "plus" },
          { label: "Dot", value: "dot" },
          { label: "Circle", value: "circle" },
          { label: "X", value: "x" },
          { label: "Square", value: "square" },
        ],
      },
      { key: "color", label: "Color", type: "color" },
      { key: "size", label: "Size", type: "range", min: 4, max: 96, step: 1 },
      { key: "thickness", label: "Thickness", type: "range", min: 1, max: 12, step: 1 },
      { key: "gap", label: "Gap", type: "range", min: 0, max: 32, step: 1 },
      { key: "opacity", label: "Opacity", type: "range", min: 0.05, max: 1, step: 0.05 },
    ],
    css: ".overlay-module-crosshair{transform:translate(-50%,-50%)}",
    wrapperClass: "",
    tick: ({ settings, api }) => {
      isFallbackCrosshairEnabled(settings, api);
    },
    visible: ({ settings, api }) => isFallbackCrosshairEnabled(settings, api),
    derive: ({ context }) => context,
    render: ({ settings }) => {
      const size = Number(settings.size ?? 24);
      const thickness = Number(settings.thickness ?? 2);
      const gap = Number(settings.gap ?? 5);
      const arm = Math.max(1, (size - gap) / 2);
      const color = settings.color ?? "#ffffff";
      const opacity = Number(settings.opacity ?? 0.9);
      const line = `position:absolute;background:${color};opacity:${opacity};box-shadow:0 0 8px rgba(0,0,0,.45);`;
      const center = size / 2 - thickness / 2;
      if (settings.style === "dot") {
        return `<div style="width:${size}px;height:${size}px;position:relative"><div style="${line}left:${center}px;top:${center}px;width:${thickness}px;height:${thickness}px;border-radius:999px"></div></div>`;
      }
      if (settings.style === "circle") {
        return `<div style="width:${size}px;height:${size}px;border:${thickness}px solid ${color};opacity:${opacity};border-radius:999px;box-shadow:0 0 8px rgba(0,0,0,.45)"></div>`;
      }
      if (settings.style === "x") {
        const xLine = `position:absolute;left:${gap / 2}px;top:${center}px;width:${Math.max(1, size - gap)}px;height:${thickness}px;background:${color};opacity:${opacity};box-shadow:0 0 8px rgba(0,0,0,.45);transform-origin:center;`;
        return `<div style="position:relative;width:${size}px;height:${size}px"><div style="${xLine}transform:rotate(45deg)"></div><div style="${xLine}transform:rotate(-45deg)"></div></div>`;
      }
      if (settings.style === "square") {
        return `<div style="width:${size}px;height:${size}px;border:${thickness}px solid ${color};opacity:${opacity};box-shadow:0 0 8px rgba(0,0,0,.45)"></div>`;
      }
      return `<div style="position:relative;width:${size}px;height:${size}px">
        <div style="${line}left:0;top:${size / 2 - thickness / 2}px;width:${arm}px;height:${thickness}px"></div>
        <div style="${line}right:0;top:${size / 2 - thickness / 2}px;width:${arm}px;height:${thickness}px"></div>
        <div style="${line}left:${size / 2 - thickness / 2}px;top:0;width:${thickness}px;height:${arm}px"></div>
        <div style="${line}left:${size / 2 - thickness / 2}px;bottom:0;width:${thickness}px;height:${arm}px"></div>
      </div>`;
    },
  },
  {
    id: "game_timer",
    fileName: "builtin:fallback-game-timer",
    name: "Game Timer",
    description: "Built-in fallback shown when overlay modules cannot be loaded.",
    locked: false,
    defaultPosition: { x: 4, y: 6 },
    defaultSettings: { enabled: false },
    settings: [{ key: "enabled", label: "Enabled", type: "boolean", default: false }],
    css: "",
    wrapperClass: "rounded border border-white/15 bg-black/70 p-3 shadow-xl shadow-black/45",
    visible: ({ settings }) => settings.enabled !== false,
    derive: ({ context }) => context,
    render: ({ context }) => `<div class="overlay-title">Game Timer</div><div class="overlay-value">${formatSeconds(context.elapsedSeconds)}</div>`,
  },
  {
    id: "image",
    fileName: "builtin:fallback-image",
    name: "Image",
    description: "Display an uploaded image on the overlay.",
    locked: false,
    defaultPosition: { x: 64, y: 16 },
    defaultSettings: {
      enabled: false,
      image: "",
      width: 240,
      opacity: 1,
      radius: 0,
    },
    settings: [
      { key: "enabled", label: "Enabled", type: "boolean", default: false },
      { key: "image", label: "Image", type: "image", default: "" },
      { key: "width", label: "Width", type: "range", min: 48, max: 900, step: 1, default: 240 },
      { key: "opacity", label: "Opacity", type: "range", min: 0.05, max: 1, step: 0.05, default: 1 },
      { key: "radius", label: "Corner Radius", type: "range", min: 0, max: 48, step: 1, default: 0 },
    ],
    css: "",
    wrapperClass: "",
    visible: ({ context, settings }) => settings.enabled !== false && (context.editMode || settings.image),
    derive: ({ context }) => context,
    render: ({ settings }) => {
      const src = String(settings.image ?? "");
      if (!src) {
        return `<div class="rounded border border-dashed border-white/25 bg-black/45 px-4 py-3 text-sm text-white/55">Upload an image</div>`;
      }
      const width = Math.max(48, Math.min(900, Number(settings.width ?? 240) || 240));
      const opacity = Math.max(0.05, Math.min(1, Number(settings.opacity ?? 1) || 1));
      const radius = Math.max(0, Math.min(48, Number(settings.radius ?? 0) || 0));
      return `<img src="${escapeHtml(src)}" alt="" style="display:block;width:${width}px;max-width:90vw;height:auto;opacity:${opacity};border-radius:${radius}px;filter:drop-shadow(0 12px 28px rgba(0,0,0,.45));" />`;
    },
  },
];
