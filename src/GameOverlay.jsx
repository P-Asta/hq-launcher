import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { FolderOpen, ImageIcon, Keyboard, Pencil, RefreshCw, Trash2, Upload, X } from "lucide-react";
import { Button } from "./components/ui/button";
import { Slider } from "./components/ui/slider";
import { cn } from "./lib/cn";

const DEFAULT_CONFIG = {
  general: {
    enabled: true,
    use_stream_overlays_api: false,
    overlay_key: "Insert",
    end_summary_duration_ms: 10000,
  },
  crosshair: {},
  widgets: {},
  module_settings: {},
  end_summary: {
    duration_ms: 10000,
  },
};

const fallbackCrosshairState = {
  runtimeEnabled: null,
  lastSettingEnabled: null,
};

function isFallbackCrosshairEnabled(settings, api) {
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

const FALLBACK_MODULES = [
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

function formatSeconds(totalSeconds) {
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

function number(value) {
  const numeric = Number(value ?? 0);
  if (!Number.isFinite(numeric)) return "0";
  return numeric.toLocaleString();
}

function stripLcQuote(value) {
  return typeof value === "string" ? value.trim().replace(/^'/, "") : value;
}

function intish(value, fallback = 0) {
  const cleaned = stripLcQuote(value);
  const numeric = Number(cleaned ?? fallback);
  return Number.isFinite(numeric) ? numeric : fallback;
}

function valueAt(root, path, fallback = undefined) {
  const parts = Array.isArray(path) ? path : String(path).split(".");
  let current = root;
  for (const part of parts) {
    if (current == null || typeof current !== "object" || !(part in current)) {
      return fallback;
    }
    current = current[part];
  }
  return current ?? fallback;
}

function valueAtAny(root, paths, fallback = undefined) {
  for (const path of paths) {
    const value = valueAt(root, path);
    if (value !== undefined && value !== null) return value;
  }
  return fallback;
}

function isStreamOverlaysPayload(value) {
  if (!value || typeof value !== "object") return false;
  return value.type === "data"
    || value.showOverlay !== undefined
    || value.crewCount !== undefined
    || value.moonName !== undefined
    || value.weatherName !== undefined
    || value.quotaValue !== undefined
    || value.quotaIndex !== undefined
    || value.lootValue !== undefined;
}

function extractStreamOverlaysPayload(value) {
  if (!value || typeof value !== "object") return null;
  const candidates = [
    value,
    value.data,
    value.payload,
    value.message,
  ];
  return candidates.find(isStreamOverlaysPayload) ?? null;
}

const HQ_FIRESTORE_PROJECT = "highquotahq214";
const HQ_FIRESTORE_API_KEY = "AIzaSyCklz28QDpVHdagTruIxlPc5hdi-fj6QxE";
const hqRunsSessionCache = new Map();
const HQ_COLLECTIONS = {
  hq: "leaderboards_hq",
  sdc: "leaderboards_sdc",
  smhq: "leaderboards_smhq",
};
const HQ_LEADERBOARD_COLLECTIONS = {
  vanilla: {
    hq: "leaderboards_hq",
    sdc: "leaderboards_sdc",
    smhq: "leaderboards_smhq",
  },
  modded: {
    hq: "modded_hq",
    sdc: "modded_sdc",
    smhq: "modded_smhq",
  },
  legacyModded: {
    brutal: {
      hq: "lc_modded_brutal_hq",
      sdc: "lc_modded_brutal_sdc",
      smhq: "lc_modded_brutal_smhq",
    },
    eclipsed: {
      hq: "lc_modded_eclipsed_hq",
      smhq: "lc_modded_eclipsed_smhq",
    },
    wesleysMoons: {
      hq: "lc_modded_wesleysmoons_hq",
      sdc: "lc_modded_wesleysmoons_sdc",
      smhq: "lc_modded_wesleysmoons_smhq",
    },
    classicMoons: {
      hq: "lc_modded_classicmoons_hq",
      sdc: "lc_modded_classicmoons_sdc",
      smhq: "lc_modded_classicmoons_smhq",
    },
  },
};
const HQ_LEADERBOARD_BOARD_TYPES = {
  hq: { id: "hq", name: "Classic High Quota", metricLabel: "Quota Amount", metricKey: "quotaAmount" },
  sdc: { id: "sdc", name: "Single Day Clear", metricLabel: "Total Scrap", metricKey: "totalScrap" },
  smhq: { id: "smhq", name: "Single Moon High Quota", metricLabel: "Quota Amount", metricKey: "quotaAmount" },
};
const HQ_LEADERBOARD_RUN_FIELDS = [
  "id",
  "collectionName",
  "players",
  "version",
  "verified",
  "quotaAmount",
  "quotaReached",
  "totalScrap",
  "moon",
  "scrapType",
  "videos",
  "date",
  "verifiedAt",
  "verifier",
];

function firestoreValue(value) {
  if (!value || typeof value !== "object") return null;
  if ("stringValue" in value) return value.stringValue;
  if ("integerValue" in value) return Number(value.integerValue);
  if ("doubleValue" in value) return Number(value.doubleValue);
  if ("booleanValue" in value) return !!value.booleanValue;
  if ("timestampValue" in value) return value.timestampValue;
  if ("arrayValue" in value) return (value.arrayValue.values ?? []).map(firestoreValue);
  if ("mapValue" in value) {
    return Object.fromEntries(
      Object.entries(value.mapValue.fields ?? {}).map(([key, item]) => [key, firestoreValue(item)]),
    );
  }
  return null;
}

function firestoreDocument(doc) {
  const data = Object.fromEntries(
    Object.entries(doc?.fields ?? {}).map(([key, value]) => [key, firestoreValue(value)]),
  );
  return { id: String(doc?.name ?? "").split("/").pop(), ...data };
}

async function fetchHqRuns(collectionName) {
  if (hqRunsSessionCache.has(collectionName)) return hqRunsSessionCache.get(collectionName);

  const url = `https://firestore.googleapis.com/v1/projects/${HQ_FIRESTORE_PROJECT}/databases/(default)/documents/${collectionName}?pageSize=1000&key=${HQ_FIRESTORE_API_KEY}`;
  const request = fetch(url)
    .then((response) => {
      if (!response.ok) throw new Error(`HighQuotaHQ fetch failed: ${response.status}`);
      return response.json();
    })
    .then((payload) => (payload.documents ?? []).map(firestoreDocument))
    .catch((error) => {
      hqRunsSessionCache.delete(collectionName);
      throw error;
    });
  hqRunsSessionCache.set(collectionName, request);
  return request;
}

function normalizeHqMoon(value) {
  const text = stripLcQuote(value).trim().replace(/\s+/, "-");
  return text || "41-Experimentation";
}

function getPlayerCountFromStats(stats) {
  const players = valueAt(stats, "Players");
  if (players && typeof players === "object") return Math.max(1, Object.keys(players).length);
  return 1;
}

function getLeaderboardInput(statsView, stats, settings = {}) {
  const isQuota = statsView?.newQuota !== 0;
  const boardType = isQuota ? "hq" : "sdc";
  const track = settings.track === "modded" ? "modded" : "vanilla";
  const metricKey = boardType === "hq" ? "quotaAmount" : "totalScrap";
  const score = boardType === "hq"
    ? intish(statsView?.newQuota)
    : intish(statsView?.totalAvailableValue || statsView?.collectedTotal);
  return {
    boardType,
    track,
    collectionName: HQ_LEADERBOARD_COLLECTIONS[track]?.[boardType] ?? HQ_COLLECTIONS[boardType],
    metricKey,
    score,
    playerCount: getPlayerCountFromStats(stats),
    version: `v${intish(valueAtAny(stats, ["Version"], 0))}`,
    moon: normalizeHqMoon(statsView?.moon ?? valueAt(stats, "MoonInfo.Name", "")),
  };
}

function normalizeVersion(value) {
  const text = String(value ?? "").trim();
  if (!text) return "";
  return text.toLowerCase().startsWith("v") ? text.toLowerCase() : `v${text}`.toLowerCase();
}

function getRunMetric(run, input) {
  if (input.boardType === "sdc") {
    return intish(run.topLine ?? run.topline ?? run.totalAvailableValue ?? run.totalScrap);
  }
  return intish(run[input.metricKey]);
}

function calculateLeaderboardCheck(runs, input, settings = {}) {
  const includeCurrentVersion = settings.includeCurrentVersion === true;
  const filtered = runs
    .filter((run) => run.verified === true)
    .filter((run) => input.boardType === "sdc" || String(run.players?.length || 0) === String(input.playerCount))
    .filter((run) => input.boardType === "hq" || !input.moon || run.moon === input.moon)
    .filter((run) => !includeCurrentVersion || normalizeVersion(run.version) === normalizeVersion(input.version))
    .sort((a, b) => getRunMetric(b, input) - getRunMetric(a, input));

  let rank = 1;
  for (const run of filtered) {
    if (getRunMetric(run, input) > input.score) rank += 1;
  }

  return {
    status: "ready",
    ...input,
    collections: HQ_LEADERBOARD_COLLECTIONS,
    boardTypes: HQ_LEADERBOARD_BOARD_TYPES,
    runFields: HQ_LEADERBOARD_RUN_FIELDS,
    totalRecords: filtered.length,
    top: filtered[0] ? {
      rank: 1,
      score: getRunMetric(filtered[0], input),
      players: filtered[0].players ?? [],
    } : null,
    includeCurrentVersion,
    metricLabel: input.boardType === "sdc" ? "Top Line" : "Score",
    next: filtered.find((run) => getRunMetric(run, input) < input.score) ?? null,
    nextScore: (() => {
      const next = filtered.find((run) => getRunMetric(run, input) < input.score);
      return next ? getRunMetric(next, input) : null;
    })(),
    rank,
  };
}

function createLcStatsView(stats) {
  if (!stats || typeof stats !== "object") return stats ?? null;
  const aliases = {
    moon: () => stripLcQuote(valueAt(stats, "MoonInfo.Name", "Unknown")),
    seed: () => stripLcQuote(valueAt(stats, "Seed", "")),
    collectedTotal: () => intish(valueAtAny(stats, ["PerformanceInfo.CollectedTotal", "CollectedTotal"])),
    collectedNoExtra: () => intish(valueAtAny(stats, ["PerformanceInfo.CollectedNoExtra", "CollectedNoExtra"])),
    initialAvailableValue: () => intish(valueAtAny(stats, [
      "PerformanceInfo.InitialAvailableValue",
      "InitialAvailableValue",
      "BottomLine",
    ])),
    totalAvailableValue: () => intish(valueAtAny(stats, [
      "PerformanceInfo.TotalAvailableValue",
      "TotalAvailableValue",
      "BottomLineTrue",
    ])),
    valueSold: () => intish(valueAtAny(stats, ["QuotaInfo.ValueSold", "ValueSold"])),
    newQuota: () => intish(valueAtAny(stats, ["QuotaInfo.NewQuota", "NewQuota"])),
    lostScrap: () => {
      const missed = Array.isArray(stats.MissedItems) ? stats.MissedItems : [];
      return missed
        .filter((item) => item && item.CollectedOnPreviousDay)
        .reduce((total, item) => total + intish(item.Value), 0);
    },
  };
  aliases.isQuotaEvent = () => aliases.newQuota() !== 0;
  aliases.isSellOrQuotaEvent = () => aliases.valueSold() !== 0 || aliases.newQuota() !== 0;

  return new Proxy(stats, {
    get(target, prop, receiver) {
      if (typeof prop === "string" && aliases[prop]) return aliases[prop]();
      return Reflect.get(target, prop, receiver);
    },
  });
}

function normalizePosition(position, fallback = { x: 50, y: 50 }) {
  const x = Number(position?.x);
  const y = Number(position?.y);
  return {
    x: Number.isFinite(x) ? Math.max(0, Math.min(100, x)) : fallback.x,
    y: Number.isFinite(y) ? Math.max(0, Math.min(100, y)) : fallback.y,
  };
}

function normalizeWidgetPosition(position, fallback = { x: 50, y: 50, snap: true }) {
  return {
    ...normalizePosition(position, fallback),
    snap: position?.snap ?? fallback.snap ?? true,
  };
}

function rectsOverlap(startA, endA, startB, endB) {
  return startA < endB && endA > startB;
}

function snapOverlayPosition(rawPosition, id, _widgets, enabled = true, metrics = null) {
  const thresholdPx = 10;
  const guides = [];
  if (!enabled) return { position: normalizePosition(rawPosition), guides };
  let next = normalizePosition(rawPosition);

  if (!metrics?.dragRect || !metrics?.viewportWidth || !metrics?.viewportHeight) {
    return { position: next, guides };
  }

  const { dragRect, otherRects = [], viewportWidth, viewportHeight } = metrics;
  const current = {
    left: (next.x / 100) * viewportWidth,
    top: (next.y / 100) * viewportHeight,
    width: dragRect.width,
    height: dragRect.height,
  };
  current.right = current.left + current.width;
  current.bottom = current.top + current.height;

  let bestX = null;
  let bestY = null;
  const considerX = (candidate) => {
    if (candidate.distance <= thresholdPx && (!bestX || candidate.distance < bestX.distance)) {
      bestX = candidate;
    }
  };
  const considerY = (candidate) => {
    if (candidate.distance <= thresholdPx && (!bestY || candidate.distance < bestY.distance)) {
      bestY = candidate;
    }
  };

  [
    { x: 0, guide: 0, distance: Math.abs(current.left) },
    { x: viewportWidth - current.width, guide: viewportWidth, distance: Math.abs(current.right - viewportWidth) },
  ].forEach(considerX);
  [
    { y: 0, guide: 0, distance: Math.abs(current.top) },
    { y: viewportHeight - current.height, guide: viewportHeight, distance: Math.abs(current.bottom - viewportHeight) },
  ].forEach(considerY);

  for (const other of otherRects) {
    if (!other || other.id === id) continue;

    const verticalOverlap = rectsOverlap(current.top, current.bottom, other.top, other.bottom);
    const horizontalOverlap = rectsOverlap(current.left, current.right, other.left, other.right);

    if (verticalOverlap) {
      const candidates = [
        { x: other.left, guide: other.left, distance: Math.abs(current.left - other.left) },
        { x: other.left - current.width, guide: other.left, distance: Math.abs(current.right - other.left) },
        { x: other.right - current.width, guide: other.right, distance: Math.abs(current.right - other.right) },
        { x: other.right, guide: other.right, distance: Math.abs(current.left - other.right) },
      ];
      candidates.forEach(considerX);
    }

    if (horizontalOverlap) {
      const candidates = [
        { y: other.top, guide: other.top, distance: Math.abs(current.top - other.top) },
        { y: other.top - current.height, guide: other.top, distance: Math.abs(current.bottom - other.top) },
        { y: other.bottom - current.height, guide: other.bottom, distance: Math.abs(current.bottom - other.bottom) },
        { y: other.bottom, guide: other.bottom, distance: Math.abs(current.top - other.bottom) },
      ];
      candidates.forEach(considerY);
    }
  }

  if (bestX) {
    next = { ...next, x: (bestX.x / viewportWidth) * 100 };
    guides.push({ axis: "x", value: bestX.guide });
  }
  if (bestY) {
    next = { ...next, y: (bestY.y / viewportHeight) * 100 };
    guides.push({ axis: "y", value: bestY.guide });
  }

  return { position: normalizePosition(next), guides };
}

function safeClassName(value) {
  return String(value ?? "")
    .toLowerCase()
    .replace(/[^a-z0-9_-]/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "");
}

function safeOverlayId(value) {
  return safeClassName(value) || "overlay";
}

function normalizeInputKeyName(value) {
  const text = String(value ?? "").trim();
  if (!text) return "";
  if (/^Key[A-Z]$/i.test(text)) return text.slice(3).toUpperCase();
  if (/^Digit[0-9]$/i.test(text)) return text.slice(5);
  if (text === " ") return "Space";
  if (text.length === 1) return text.toUpperCase();
  const aliases = {
    control: "Ctrl",
    ctrl: "Ctrl",
    escape: "Escape",
    esc: "Escape",
    command: "Meta",
    cmd: "Meta",
    win: "Meta",
    windows: "Meta",
    option: "Alt",
    return: "Enter",
  };
  return aliases[text.toLowerCase()] ?? text;
}

function shortcutParts(value) {
  return String(value ?? "")
    .split("+")
    .map((part) => normalizeInputKeyName(part))
    .filter(Boolean);
}

function canonicalShortcut(value) {
  const parts = shortcutParts(value);
  const modifiers = [];
  if (parts.some((part) => part === "Ctrl")) modifiers.push("Ctrl");
  if (parts.some((part) => part === "Shift")) modifiers.push("Shift");
  if (parts.some((part) => part === "Alt")) modifiers.push("Alt");
  if (parts.some((part) => part === "Meta")) modifiers.push("Meta");
  const key = parts.find((part) => !["Ctrl", "Shift", "Alt", "Meta"].includes(part)) ?? "";
  return [...modifiers, key].filter(Boolean).join("+");
}

function eventShortcutFromKeyboardEvent(event) {
  const key = normalizeInputKeyName(event.key || event.code);
  if (!key || ["Ctrl", "Shift", "Alt", "Meta"].includes(key)) return "";
  return [
    event.ctrlKey ? "Ctrl" : "",
    event.shiftKey ? "Shift" : "",
    event.altKey ? "Alt" : "",
    event.metaKey ? "Meta" : "",
    key,
  ].filter(Boolean).join("+");
}

function createInputSnapshot() {
  return {
    down: new Set(),
    events: [],
    sequence: 0,
  };
}

function matchesInputShortcut(event, shortcut) {
  const wanted = canonicalShortcut(shortcut);
  if (!wanted) return false;
  if (event.shortcut === wanted) return true;
  const wantedParts = shortcutParts(wanted);
  const wantedKey = wantedParts.find((part) => !["Ctrl", "Shift", "Alt", "Meta"].includes(part));
  return event.key === wantedKey
    && !!event.ctrlKey === wantedParts.includes("Ctrl")
    && !!event.shiftKey === wantedParts.includes("Shift")
    && !!event.altKey === wantedParts.includes("Alt")
    && !!event.metaKey === wantedParts.includes("Meta");
}

function createInputApi(moduleId, inputSnapshotRef, consumedInputRef) {
  function current() {
    return inputSnapshotRef.current ?? createInputSnapshot();
  }

  function consume(type, shortcut) {
    const snapshot = current();
    const event = snapshot.events.find((item) => item.type === type && matchesInputShortcut(item, shortcut));
    if (!event) return false;
    const key = `${moduleId}:${type}:${canonicalShortcut(shortcut)}:${event.id}`;
    if (consumedInputRef.current.has(key)) return false;
    consumedInputRef.current.add(key);
    if (consumedInputRef.current.size > 512) {
      const first = consumedInputRef.current.values().next().value;
      consumedInputRef.current.delete(first);
    }
    return true;
  }

  return {
    down: (shortcut) => current().down.has(canonicalShortcut(shortcut)),
    held: (shortcut) => current().down.has(canonicalShortcut(shortcut)),
    shortcut: (shortcut) => current().down.has(canonicalShortcut(shortcut)),
    pressed: (shortcut) => current().events.some((event) => event.type === "keydown" && matchesInputShortcut(event, shortcut)),
    released: (shortcut) => current().events.some((event) => event.type === "keyup" && matchesInputShortcut(event, shortcut)),
    consumePress: (shortcut) => consume("keydown", shortcut),
    consumeRelease: (shortcut) => consume("keyup", shortcut),
    events: () => current().events.slice(),
    last: () => current().events[0] ?? null,
  };
}

function createModuleApi(
  moduleId,
  inputSnapshotRef = { current: createInputSnapshot() },
  consumedInputRef = { current: new Set() },
  contextRef = { current: null },
) {
  return {
    id: moduleId,
    formatSeconds,
    escapeHtml,
    html: escapeHtml,
    number,
    stripLcQuote,
    intish,
    valueAt,
    valueAtAny,
    className: (name) => `overlay-module-${safeClassName(moduleId)} ${name ? safeClassName(name) : ""}`.trim(),
    now: () => Date.now(),
    input: createInputApi(moduleId, inputSnapshotRef, consumedInputRef),
    get context() {
      return contextRef.current;
    },
    getLcStats: () => contextRef.current?.lcstats ?? null,
    getLcStatsRaw: () => contextRef.current?.lcstatsRaw ?? null,
    getStreamOverlay: () => contextRef.current?.streamOverlays ?? null,
  };
}

function normalizeSettingsSchema(items) {
  return (Array.isArray(items) ? items : [])
    .filter((item) => item && item.key)
    .map((item) => ({ ...item, key: String(item.key) }));
}

function defaultSettingsFromSchema(items) {
  return normalizeSettingsSchema(items).reduce((settings, item) => {
    if (item.default !== undefined) {
      settings[item.key] = item.default;
    } else if (item.type === "boolean") {
      settings[item.key] = false;
    } else if (item.type === "color") {
      settings[item.key] = "#ffffff";
    } else if (item.type === "select") {
      settings[item.key] = item.options?.[0]?.value ?? "";
    } else if (item.type === "number") {
      settings[item.key] = item.default ?? item.min ?? 0;
    } else if (item.type === "images") {
      settings[item.key] = [];
    } else if (item.type === "key" || item.type === "image") {
      settings[item.key] = "";
    } else {
      settings[item.key] = item.min ?? "";
    }
    return settings;
  }, {});
}

function createCtRuntime(raw) {
  const id = safeOverlayId(raw.id || raw.file_name?.replace(/\.js$/i, "") || "");
  const fileName = raw.file_name || `${id}.js`;

  const Setting = {
    toggle: (key, label, defaultValue = false) => ({ key, label, type: "boolean", default: defaultValue }),
    color: (key, label, defaultValue = "#ffffff") => ({ key, label, type: "color", default: defaultValue }),
    range: (key, label, min, max, step = 1, defaultValue = min) => ({
      key,
      label,
      type: "range",
      min,
      max,
      step,
      default: defaultValue,
    }),
    text: (key, label, defaultValue = "") => ({ key, label, type: "text", default: defaultValue }),
    textarea: (key, label, defaultValue = "") => ({ key, label, type: "textarea", default: defaultValue }),
    image: (key, label, defaultValue = "") => ({ key, label, type: "image", default: defaultValue }),
    images: (key, label, defaultValue = []) => ({ key, label, type: "images", default: defaultValue }),
    key: (key, label, defaultValue = "") => ({ key, label, type: "key", default: defaultValue }),
    number: (key, label, defaultValue = 0, min = undefined, max = undefined, step = 1) => ({
      key,
      label,
      type: "number",
      min,
      max,
      step,
      default: defaultValue,
    }),
    select: (key, label, options, defaultValue) => ({
      key,
      label,
      type: "select",
      options,
      default: defaultValue ?? options?.[0]?.value ?? "",
    }),
  };
  Setting.selectMenu = Setting.select;
  Setting.hotkey = Setting.key;

  function createInstance(instanceId, displayId = instanceId) {
    const meta = {
      id: instanceId,
      fileName,
      name: displayId,
      description: "",
      locked: false,
      defaultPosition: { x: 50, y: 50 },
      settings: [],
      defaultSettings: {},
      css: "",
      wrapperClass: "",
    };
    const handlers = {
      visible: [],
      derive: [],
      renderOverlay: [],
      tick: [],
      lcstats: [],
    };

    function register(type, payload) {
      if (type === "metadata" && payload && typeof payload === "object") {
        if (payload.name) meta.name = String(payload.name);
        if (payload.description) meta.description = String(payload.description);
        if (payload.locked !== undefined) meta.locked = !!payload.locked;
        if (payload.defaultPosition) meta.defaultPosition = normalizePosition(payload.defaultPosition);
        if (payload.wrapperClass) meta.wrapperClass = String(payload.wrapperClass);
        return api;
      }
      if (type === "settings") {
        meta.settings = normalizeSettingsSchema(payload);
        meta.defaultSettings = {
          ...meta.defaultSettings,
          ...defaultSettingsFromSchema(meta.settings),
        };
        return api;
      }
      if (type === "defaults" && payload && typeof payload === "object") {
        meta.defaultSettings = { ...meta.defaultSettings, ...payload };
        return api;
      }
      if (type === "css") {
        meta.css += `${meta.css ? "\n" : ""}${String(payload ?? "")}`;
        return api;
      }
      if (handlers[type] && typeof payload === "function") {
        handlers[type].push(payload);
        return api;
      }
      throw new Error(`Unknown overlay register type: ${type}`);
    }

    const api = {
      register,
      Setting,
      setName: (name) => {
        meta.name = String(name);
        return api;
      },
      setDescription: (description) => {
        meta.description = String(description);
        return api;
      },
      setLocked: (locked = true) => {
        meta.locked = !!locked;
        return api;
      },
      setDefaultPosition: (position) => {
        meta.defaultPosition = normalizePosition(position);
        return api;
      },
      setDefaultSettings: (settings) => {
        meta.defaultSettings = { ...meta.defaultSettings, ...(settings ?? {}) };
        return api;
      },
      setWrapperClass: (wrapperClass) => {
        meta.wrapperClass = String(wrapperClass ?? "");
        return api;
      },
      setCss: (css) => register("css", css),
    };

    return { id: instanceId, meta, handlers, api };
  }

  const instance = createInstance(id, id);
  return { id, instance, api: instance.api, Setting };
}

function evaluateModule(raw) {
  try {
    const runtime = createCtRuntime(raw);
    const helpers = createModuleApi(runtime.id);
    const script = Function(
      "register",
      "Setting",
      "setName",
      "setDescription",
      "setLocked",
      "setDefaultPosition",
      "setDefaultSettings",
      "setWrapperClass",
      "setCss",
      "api",
      "html",
      "formatSeconds",
      "number",
      "valueAt",
      "valueAtAny",
      "intish",
      `"use strict";\n${raw.source}`,
    );
    script(
      runtime.api.register,
      runtime.api.Setting,
      runtime.api.setName,
      runtime.api.setDescription,
      runtime.api.setLocked,
      runtime.api.setDefaultPosition,
      runtime.api.setDefaultSettings,
      runtime.api.setWrapperClass,
      runtime.api.setCss,
      helpers,
      escapeHtml,
      formatSeconds,
      number,
      valueAt,
      valueAtAny,
      intish,
    );
    return [runtime.instance]
      .map((instance) => {
        const hasRegisteredHandlers =
          instance.handlers.visible.length > 0 ||
          instance.handlers.derive.length > 0 ||
          instance.handlers.renderOverlay.length > 0 ||
          instance.meta.settings.length > 0 ||
          instance.meta.css.trim();
        if (!hasRegisteredHandlers) return null;

        const settings = normalizeSettingsSchema(instance.meta.settings);
        return {
          id: instance.meta.id,
          fileName: instance.meta.fileName,
          name: instance.meta.name,
          description: instance.meta.description,
          locked: instance.meta.locked,
          defaultPosition: normalizePosition(instance.meta.defaultPosition),
          defaultSettings: {
            ...defaultSettingsFromSchema(settings),
            ...instance.meta.defaultSettings,
          },
          settings,
          css: instance.meta.css,
          wrapperClass: instance.meta.wrapperClass,
          visible: (ctx) => instance.handlers.visible.every((handler) => handler(ctx) !== false),
          derive: (ctx) => {
            let data = ctx.context;
            for (const handler of instance.handlers.derive) {
              const next = handler({ ...ctx, data });
              if (next !== undefined) data = next;
            }
            return data;
          },
          tick: (ctx) => {
            for (const handler of instance.handlers.tick) {
              handler(ctx);
            }
          },
          render: (ctx) => {
            const rendered = instance.handlers.renderOverlay.map((handler) => handler(ctx));
            if (rendered.length === 1 && Array.isArray(rendered[0])) return rendered[0];
            return rendered.flatMap((item) => (Array.isArray(item) ? item : [item])).join("");
          },
        };
      })
      .filter(Boolean);
  } catch (error) {
    console.error(`Failed to load overlay module ${raw.file_name ?? raw.id}`, error);
    return [];
  }
}

function normalizeConfig(config, modules) {
  const next = {
    ...DEFAULT_CONFIG,
    ...(config ?? {}),
    general: {
      ...DEFAULT_CONFIG.general,
      ...(config?.general ?? {}),
    },
    widgets: { ...(config?.widgets ?? {}) },
    module_settings: { ...(config?.module_settings ?? {}) },
  };

  for (const module of modules) {
    next.widgets[module.id] = normalizeWidgetPosition(next.widgets[module.id], {
      ...module.defaultPosition,
      snap: module.locked ? false : true,
    });
    next.module_settings[module.id] = {
      ...module.defaultSettings,
      ...(next.module_settings[module.id] ?? {}),
    };
  }

  return next;
}

function settingValue(settings, item) {
  if (settings[item.key] !== undefined) return settings[item.key];
  if (item.default !== undefined) return item.default;
  if (item.type === "boolean") return false;
  if (item.type === "color") return "#ffffff";
  if (item.type === "select") return item.options?.[0]?.value ?? "";
  if (item.type === "number") return item.min ?? 0;
  if (item.type === "images") return [];
  if (item.type === "key" || item.type === "image") return "";
  return item.min ?? "";
}

function resetValueForSetting(item) {
  if (item.default !== undefined) return item.default;
  if (item.type === "boolean") return false;
  if (item.type === "color") return "#ffffff";
  if (item.type === "select") return item.options?.[0]?.value ?? "";
  if (item.type === "number") return item.min ?? 0;
  if (item.type === "images") return [];
  if (item.type === "key" || item.type === "image") return "";
  return item.min ?? "";
}

function collectModuleInputShortcuts(modules, config) {
  const shortcuts = new Set();
  for (const module of modules) {
    const settings = config.module_settings?.[module.id] ?? module.defaultSettings ?? {};
    for (const item of module.settings ?? []) {
      if (item.type !== "key") continue;
      const shortcut = canonicalShortcut(settings[item.key] ?? item.default);
      if (shortcut) shortcuts.add(shortcut);
    }
  }
  return Array.from(shortcuts);
}

function normalizeShortcutBaseKey(event) {
  if (event.code?.startsWith("Numpad")) return event.code;
  if (event.key === " ") return "Space";
  if (event.key?.length === 1) return event.key.toUpperCase();
  return event.key || "";
}

function normalizeKeyInput(event) {
  const baseKey = normalizeShortcutBaseKey(event);
  if (!baseKey || ["Control", "Shift", "Alt", "Meta", "OS"].includes(baseKey)) return "";

  const modifiers = [];
  if (event.ctrlKey) modifiers.push("Ctrl");
  if (event.shiftKey) modifiers.push("Shift");
  if (event.altKey) modifiers.push("Alt");
  if (event.metaKey) modifiers.push("Meta");

  return [...modifiers, baseKey].join("+");
}

function KeyCaptureButton({ active, value, onStart, onCancel, onCapture }) {
  useEffect(() => {
    if (!active) return undefined;

    const handleKeyDown = (event) => {
      event.preventDefault();
      event.stopPropagation();
      const nextKey = normalizeKeyInput(event);
      if (!nextKey) return;
      onCapture(nextKey);
    };
    const handlePointerDown = (event) => {
      if (event.target?.closest?.("[data-key-capture-button='true']")) return;
      onCancel();
    };

    window.addEventListener("keydown", handleKeyDown, true);
    window.addEventListener("pointerdown", handlePointerDown, true);
    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
      window.removeEventListener("pointerdown", handlePointerDown, true);
    };
  }, [active, onCancel, onCapture]);

  const displayValue = String(value || "");
  return (
    <button
      type="button"
      data-key-capture-button="true"
      onClick={onStart}
      className={cn(
        "flex h-9 w-full items-center justify-between rounded border bg-black/25 px-3 text-left text-sm text-white outline-none transition",
        active ? "border-[var(--theme-accent)] shadow-[0_0_0_2px_rgba(255,255,255,0.08)]" : "border-white/10",
      )}
    >
      <span className={displayValue || active ? "text-white" : "text-white/35"}>
        {active ? "Press shortcut..." : displayValue || "Click to record"}
      </span>
      <span className="text-[11px] text-white/35">{active ? "Recording" : "Key"}</span>
    </button>
  );
}

function ResetButton({ onClick }) {
  return (
    <button
      type="button"
      className="rounded border border-white/10 bg-white/5 px-2 py-1 text-[11px] text-white/55 hover:bg-white/10 hover:text-white"
      onClick={onClick}
    >
      Reset
    </button>
  );
}

function ImageSettingInput({ value, onChange, onReset }) {
  const fileInputRef = useRef(null);
  const fileDialogActiveRef = useRef(false);
  const hasImage = typeof value === "string" && value.startsWith("data:image/");

  useEffect(() => {
    return () => {
      if (fileDialogActiveRef.current) {
        invoke("set_game_overlay_file_dialog_active", { active: false }).catch(console.error);
      }
    };
  }, []);

  function setFileDialogActive(active) {
    fileDialogActiveRef.current = active;
    invoke("set_game_overlay_file_dialog_active", { active }).catch(console.error);
  }

  function pickImage() {
    setFileDialogActive(true);
    const clearAfterFocus = () => {
      window.setTimeout(() => {
        if (fileDialogActiveRef.current) {
          setFileDialogActive(false);
        }
      }, 250);
    };
    window.addEventListener("focus", clearAfterFocus, { once: true });
    fileInputRef.current?.click();
  }

  function handleFileChange(event) {
    setFileDialogActive(false);
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file || !file.type.startsWith("image/")) return;

    const reader = new FileReader();
    reader.onload = () => {
      if (typeof reader.result === "string") {
        onChange(reader.result);
      }
    };
    reader.readAsDataURL(file);
  }

  return (
    <div className="rounded border border-white/10 bg-black/20 p-3">
      <input
        ref={fileInputRef}
        type="file"
        accept="image/*"
        onChange={handleFileChange}
        className="hidden"
      />
      <div className="mb-3 flex items-center justify-between gap-3">
        <div className="flex items-center gap-2 text-sm text-white/80">
          <ImageIcon className="h-4 w-4 text-white/45" />
          <span>{hasImage ? "Image selected" : "No image selected"}</span>
        </div>
        <ResetButton onClick={onReset} />
      </div>
      {hasImage ? (
        <div className="mb-3 overflow-hidden rounded border border-white/10 bg-black/35">
          <img src={value} alt="" className="max-h-40 w-full object-contain" />
        </div>
      ) : null}
      <div className="flex gap-2">
        <Button type="button" variant="secondary" size="sm" className="h-9 flex-1 rounded" onClick={pickImage}>
          <Upload className="h-4 w-4" />
          Upload
        </Button>
        {hasImage ? (
          <Button type="button" variant="secondary" size="sm" className="h-9 rounded px-3" onClick={() => onChange("")}>
            <Trash2 className="h-4 w-4" />
          </Button>
        ) : null}
      </div>
    </div>
  );
}

function ImagesSettingInput({ value, onChange, onReset }) {
  const fileInputRef = useRef(null);
  const fileDialogActiveRef = useRef(false);
  const images = Array.isArray(value) ? value.filter((item) => typeof item === "string" && item.startsWith("data:image/")) : [];

  useEffect(() => {
    return () => {
      if (fileDialogActiveRef.current) {
        invoke("set_game_overlay_file_dialog_active", { active: false }).catch(console.error);
      }
    };
  }, []);

  function setFileDialogActive(active) {
    fileDialogActiveRef.current = active;
    invoke("set_game_overlay_file_dialog_active", { active }).catch(console.error);
  }

  function pickImages() {
    setFileDialogActive(true);
    const clearAfterFocus = () => {
      window.setTimeout(() => {
        if (fileDialogActiveRef.current) {
          setFileDialogActive(false);
        }
      }, 250);
    };
    window.addEventListener("focus", clearAfterFocus, { once: true });
    fileInputRef.current?.click();
  }

  function handleFileChange(event) {
    setFileDialogActive(false);
    const files = Array.from(event.target.files ?? []).filter((file) => file.type.startsWith("image/"));
    event.target.value = "";
    if (files.length === 0) return;

    Promise.all(
      files.map(
        (file) =>
          new Promise((resolve) => {
            const reader = new FileReader();
            reader.onload = () => resolve(typeof reader.result === "string" ? reader.result : null);
            reader.onerror = () => resolve(null);
            reader.readAsDataURL(file);
          }),
      ),
    ).then((nextImages) => {
      onChange([...images, ...nextImages.filter(Boolean)]);
    });
  }

  function removeImage(index) {
    onChange(images.filter((_, itemIndex) => itemIndex !== index));
  }

  return (
    <div className="rounded border border-white/10 bg-black/20 p-3">
      <input
        ref={fileInputRef}
        type="file"
        accept="image/*"
        multiple
        onChange={handleFileChange}
        className="hidden"
      />
      <div className="mb-3 flex items-center justify-between gap-3">
        <div className="flex items-center gap-2 text-sm text-white/80">
          <ImageIcon className="h-4 w-4 text-white/45" />
          <span>{images.length > 0 ? `${images.length} images` : "No images selected"}</span>
        </div>
        <ResetButton onClick={onReset} />
      </div>
      {images.length > 0 ? (
        <div className="mb-3 grid grid-cols-3 gap-2">
          {images.map((src, index) => (
            <div key={`${src.slice(0, 32)}-${index}`} className="group relative overflow-hidden rounded border border-white/10 bg-black/35">
              <img src={src} alt="" className="aspect-square w-full object-contain" />
              <button
                type="button"
                className="absolute right-1 top-1 rounded bg-black/70 p-1 text-white/70 opacity-0 hover:text-white group-hover:opacity-100"
                onClick={() => removeImage(index)}
              >
                <Trash2 className="h-3.5 w-3.5" />
              </button>
            </div>
          ))}
        </div>
      ) : null}
      <Button type="button" variant="secondary" size="sm" className="h-9 w-full rounded" onClick={pickImages}>
        <Upload className="h-4 w-4" />
        Upload
      </Button>
    </div>
  );
}

function ModuleSettings({ module, settings, onChange, onPreview, onReset }) {
  const [activeKeyInput, setActiveKeyInput] = useState(null);

  if (!module) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-white/45">
        Select a module.
      </div>
    );
  }

  if (module.settings.length === 0) {
    return <div className="text-sm text-white/45">This module has no settings.</div>;
  }

  return (
    <div className="space-y-4">
      {module.settings.map((item) => {
        const value = settingValue(settings, item);
        if (item.type === "boolean") {
          return (
            <label
              key={item.key}
              className="flex items-center justify-between gap-3 rounded border border-white/10 bg-black/20 px-3 py-2"
            >
              <span className="text-sm text-white/80">{item.label || item.key}</span>
              <div className="flex items-center gap-2">
                <ResetButton onClick={() => onReset(item)} />
                <input
                  type="checkbox"
                  checked={!!value}
                  onChange={(event) => onChange(item.key, event.target.checked)}
                  className="h-4 w-4 accent-[var(--theme-accent)]"
                />
              </div>
            </label>
          );
        }

        if (item.type === "color") {
          return (
            <div key={item.key}>
              <div className="mb-2 flex items-center justify-between gap-3 text-xs text-white/55">
                <span>{item.label || item.key}</span>
                <ResetButton onClick={() => onReset(item)} />
              </div>
              <input
                type="color"
                value={String(value)}
                onChange={(event) => onChange(item.key, event.target.value)}
                className="h-9 w-16 rounded border border-white/15 bg-transparent p-0"
              />
            </div>
          );
        }

        if (item.type === "range") {
          const numeric = Number(value);
          return (
            <div key={item.key}>
              <div className="mb-2 flex items-center justify-between gap-3 text-xs text-white/55">
                <span>{item.label || item.key}</span>
                <span className="flex items-center gap-2">
                  <span className="tabular-nums text-white/40">{Number.isFinite(numeric) ? numeric : item.min}</span>
                  <ResetButton onClick={() => onReset(item)} />
                </span>
              </div>
              <Slider
                value={[Number.isFinite(numeric) ? numeric : Number(item.min ?? 0)]}
                min={Number(item.min ?? 0)}
                max={Number(item.max ?? 100)}
                step={Number(item.step ?? 1)}
                onValueChange={([next]) => onPreview(item.key, next)}
                onValueCommit={([next]) => onChange(item.key, next)}
              />
            </div>
          );
        }

        if (item.type === "number") {
          const numeric = Number(value);
          return (
            <div key={item.key}>
              <div className="mb-2 flex items-center justify-between gap-3 text-xs text-white/55">
                <span>{item.label || item.key}</span>
                <ResetButton onClick={() => onReset(item)} />
              </div>
              <input
                type="number"
                value={Number.isFinite(numeric) ? numeric : Number(item.default ?? item.min ?? 0)}
                min={item.min}
                max={item.max}
                step={item.step ?? 1}
                onChange={(event) => onChange(item.key, Number(event.target.value))}
                className="h-9 w-full rounded border border-white/10 bg-black/25 px-3 text-sm text-white outline-none"
              />
            </div>
          );
        }

        if (item.type === "select") {
          return (
            <div key={item.key}>
              <div className="mb-2 flex items-center justify-between gap-3 text-xs text-white/55">
                <span>{item.label || item.key}</span>
                <ResetButton onClick={() => onReset(item)} />
              </div>
              <select
                value={String(value ?? "")}
                onChange={(event) => onChange(item.key, event.target.value)}
                className="h-9 w-full rounded border border-white/10 bg-[#191b22] px-3 text-sm text-white outline-none"
              >
                {(item.options ?? []).map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label || option.value}
                  </option>
                ))}
              </select>
            </div>
          );
        }

        if (item.type === "textarea") {
          return (
            <div key={item.key}>
              <div className="mb-2 flex items-center justify-between gap-3 text-xs text-white/55">
                <span>{item.label || item.key}</span>
                <ResetButton onClick={() => onReset(item)} />
              </div>
              <textarea
                value={String(value ?? "")}
                onChange={(event) => onChange(item.key, event.target.value)}
                rows={4}
                className="w-full resize-none rounded border border-white/10 bg-black/25 px-3 py-2 text-sm text-white outline-none"
              />
            </div>
          );
        }

        if (item.type === "key") {
          const isActive = activeKeyInput === item.key;
          return (
            <div key={item.key}>
              <div className="mb-2 flex items-center justify-between gap-3 text-xs text-white/55">
                <span>{item.label || item.key}</span>
                <span className="flex items-center gap-2">
                  {isActive ? <span className="text-[var(--theme-accent)]">Listening...</span> : null}
                  <ResetButton onClick={() => onReset(item)} />
                </span>
              </div>
              <KeyCaptureButton
                active={isActive}
                value={value}
                onStart={() => setActiveKeyInput(item.key)}
                onCancel={() => setActiveKeyInput(null)}
                onCapture={(nextKey) => {
                  onChange(item.key, nextKey);
                  setActiveKeyInput(null);
                }}
              />
            </div>
          );
        }

        if (item.type === "image") {
          return (
            <div key={item.key}>
              <div className="mb-2 flex items-center justify-between gap-3 text-xs text-white/55">
                <span>{item.label || item.key}</span>
              </div>
              <ImageSettingInput
                value={String(value ?? "")}
                onChange={(next) => onChange(item.key, next)}
                onReset={() => onReset(item)}
              />
            </div>
          );
        }

        if (item.type === "images") {
          return (
            <div key={item.key}>
              <div className="mb-2 flex items-center justify-between gap-3 text-xs text-white/55">
                <span>{item.label || item.key}</span>
              </div>
              <ImagesSettingInput
                value={value}
                onChange={(next) => onChange(item.key, next)}
                onReset={() => onReset(item)}
              />
            </div>
          );
        }

        return (
          <div key={item.key}>
            <div className="mb-2 flex items-center justify-between gap-3 text-xs text-white/55">
              <span>{item.label || item.key}</span>
              <ResetButton onClick={() => onReset(item)} />
            </div>
            <input
              value={String(value ?? "")}
              onChange={(event) => onChange(item.key, event.target.value)}
              className="h-9 w-full rounded border border-white/10 bg-black/25 px-3 text-sm text-white outline-none"
            />
          </div>
        );
      })}
    </div>
  );
}

function isInteractiveDragTarget(target) {
  return !!target?.closest?.("button,a,input,select,textarea,[role='button'],[data-no-drag='true']");
}

function overlayRenderEntries(module, rendered) {
  const items = Array.isArray(rendered) ? rendered : [{ html: rendered }];
  return items
    .map((item, index) => {
      const entry = item && typeof item === "object" && !Array.isArray(item)
        ? item
        : { html: item };
      const childId = safeOverlayId(entry.id ?? index + 1);
      const widgetId = Array.isArray(rendered) ? `${module.id}:${childId}` : module.id;
      return {
        module,
        widgetId,
        renderId: widgetId,
        html: String(entry.html ?? ""),
        defaultPosition: normalizePosition(entry.defaultPosition, module.defaultPosition),
      };
    })
    .filter((entry) => entry.html);
}

function OverlayModuleView({ entry, position, editMode, onDragStart }) {
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

export default function GameOverlay({ captureOnly = false }) {
  if (typeof window !== "undefined" && !window.__hqGameOverlayEntered) {
    window.__hqGameOverlayEntered = true;
    invoke("report_game_overlay_frontend_info", { message: "GameOverlay function entered" }).catch(console.error);
  }

  const [rawModules, setRawModules] = useState([]);
  const loadedModules = useMemo(() => rawModules.flatMap(evaluateModule), [rawModules]);
  const modules = loadedModules.length > 0 ? loadedModules : FALLBACK_MODULES;
  const [config, setConfig] = useState(DEFAULT_CONFIG);
  const [overlayActive, setOverlayActive] = useState(true);
  const [controlsOpen, setControlsOpen] = useState(false);
  const [editMode, setEditMode] = useState(false);
  const [selectedModuleId, setSelectedModuleId] = useState("general");
  const [moduleLoadError, setModuleLoadError] = useState("");
  const [overlayDebug, setOverlayDebug] = useState(null);
  const [lcStatsPayload, setLcStatsPayload] = useState(null);
  const [lcStatsAt, setLcStatsAt] = useState(null);
  const [streamOverlaysPayload, setStreamOverlaysPayload] = useState(null);
  const [streamOverlaysAt, setStreamOverlaysAt] = useState(null);
  const [leaderboard, setLeaderboard] = useState({
    status: "idle",
    collections: HQ_LEADERBOARD_COLLECTIONS,
    boardTypes: HQ_LEADERBOARD_BOARD_TYPES,
    runFields: HQ_LEADERBOARD_RUN_FIELDS,
  });
  const [endSummary, setEndSummary] = useState(null);
  const [overlayEvents, setOverlayEvents] = useState([]);
  const [overlayLogs, setOverlayLogs] = useState([]);
  const [openConfigHint, setOpenConfigHint] = useState(null);
  const [inputSequence, setInputSequence] = useState(0);
  const [elapsedSeconds, setElapsedSeconds] = useState(0);
  const [snapGuides, setSnapGuides] = useState([]);
  const [controlsPosition, setControlsPosition] = useState({ right: 24, top: 24 });
  const [generalKeyListening, setGeneralKeyListening] = useState(false);
  const dragRef = useRef(null);
  const controlsDragRef = useRef(null);
  const latestConfigRef = useRef(DEFAULT_CONFIG);
  const renderReportRef = useRef("");
  const inputSnapshotRef = useRef(createInputSnapshot());
  const consumedInputRef = useRef(new Set());
  const overlayContextRef = useRef(null);
  const configVersionRef = useRef(0);
  const openConfigHintTimeoutRef = useRef(null);

  function pushOverlayLog(level, message, details = undefined) {
    const entry = {
      id: `${Date.now()}-${Math.random().toString(16).slice(2)}`,
      at: Date.now(),
      level,
      message: String(message ?? ""),
      details,
    };
    setOverlayLogs((current) => [entry, ...current].slice(0, 160));
  }

  function pushInputEvent(event) {
    inputSnapshotRef.current = {
      down: event.down ?? inputSnapshotRef.current.down,
      events: [event, ...inputSnapshotRef.current.events].slice(0, 80),
      sequence: event.sequence,
    };
    setOverlayEvents((current) => [{ type: "input", ...event }, ...current].slice(0, 40));
    setInputSequence(event.sequence);
    window.setTimeout(() => {
      if (!inputSnapshotRef.current.events.some((item) => item.id === event.id)) return;
      inputSnapshotRef.current = {
        ...inputSnapshotRef.current,
        events: inputSnapshotRef.current.events.filter((item) => item.id !== event.id),
        sequence: inputSnapshotRef.current.sequence + 1,
      };
      setInputSequence(inputSnapshotRef.current.sequence);
    }, 250);
  }

  function showOpenConfigHint(payload = {}) {
    const shortcut = String(payload.shortcut ?? latestConfigRef.current?.general?.overlay_key ?? "Insert");
    const duration = Math.max(2000, Math.min(15000, Number(payload.durationMs ?? payload.duration_ms ?? 8000)));
    if (openConfigHintTimeoutRef.current) {
      window.clearTimeout(openConfigHintTimeoutRef.current);
    }
    const id = Date.now();
    setOpenConfigHint({ id, shortcut });
    openConfigHintTimeoutRef.current = window.setTimeout(() => {
      setOpenConfigHint((current) => (current?.id === id ? null : current));
      openConfigHintTimeoutRef.current = null;
    }, duration);
  }

  useEffect(() => {
    latestConfigRef.current = config;
  }, [config]);

  useEffect(() => {
    function reportFrontendError(message) {
      invoke("report_game_overlay_frontend_error", { message }).catch(console.error);
    }
    function handleError(event) {
      reportFrontendError(`${event.message ?? "window error"} at ${event.filename ?? "unknown"}:${event.lineno ?? 0}`);
    }
    function handleRejection(event) {
      reportFrontendError(`unhandled rejection: ${event.reason?.message ?? event.reason ?? "unknown"}`);
    }
    window.addEventListener("error", handleError);
    window.addEventListener("unhandledrejection", handleRejection);
    invoke("report_game_overlay_frontend_ready").catch(console.error);
    Promise.all([
      invoke("get_game_overlay_modules"),
      invoke("get_game_overlay_config"),
      invoke("get_lcstats_latest_payload").catch(() => null),
      invoke("get_game_overlay_debug_status").catch(() => null),
    ])
      .then(([nextModules, nextConfig, latestLcStats, debugStatus]) => {
        const loaded = Array.isArray(nextModules) ? nextModules : [];
        const evaluated = loaded.flatMap(evaluateModule);
        invoke("report_game_overlay_frontend_info", {
          message: `modules loaded=${loaded.length} [${loaded.map((module) => module.file_name || module.id).join(", ")}], evaluated=${evaluated.length} [${evaluated.map((module) => module.id).join(", ")}]`,
        }).catch(console.error);
        setRawModules(loaded);
        setModuleLoadError(evaluated.length === 0 ? "No overlay modules loaded. Built-in fallback is active." : "");
        setConfig(normalizeConfig(nextConfig, evaluated.length > 0 ? evaluated : FALLBACK_MODULES));
        if (debugStatus) {
          setOverlayDebug(debugStatus);
          setControlsOpen(captureOnly ? false : !!debugStatus.controlsOpen);
        }
        if (latestLcStats?.stats) {
          setLcStatsPayload({
            source: "lcstatstracker",
            receivedAt: latestLcStats.receivedAt ?? latestLcStats.received_at,
            raw: latestLcStats.raw,
            stats: latestLcStats.stats,
          });
          setLcStatsAt(Date.now());
        }
        setSelectedModuleId("general");
      })
      .catch((error) => {
        console.error(error);
        setModuleLoadError(String(error));
        setRawModules([]);
        setConfig((current) => normalizeConfig(current, FALLBACK_MODULES));
        setSelectedModuleId("general");
      });
    return () => {
      if (openConfigHintTimeoutRef.current) {
        window.clearTimeout(openConfigHintTimeoutRef.current);
        openConfigHintTimeoutRef.current = null;
      }
      window.removeEventListener("error", handleError);
      window.removeEventListener("unhandledrejection", handleRejection);
    };
  }, [captureOnly]);

  useEffect(() => {
    setConfig((current) => normalizeConfig(current, modules));
  }, [captureOnly, modules]);

  useEffect(() => {
    const interval = window.setInterval(() => {
      setElapsedSeconds((current) => current + 1);
    }, 1000);
    return () => window.clearInterval(interval);
  }, []);

  useEffect(() => {
    function handleKeyDown(event) {
      if (generalKeyListening || event.repeat || isInteractiveDragTarget(event.target)) return;
      const shortcut = canonicalShortcut(eventShortcutFromKeyboardEvent(event));
      if (!shortcut) return;
      const down = new Set(inputSnapshotRef.current.down);
      down.add(shortcut);
      pushInputEvent({
        id: `${Date.now()}-${inputSnapshotRef.current.sequence + 1}`,
        type: "keydown",
        key: normalizeInputKeyName(event.key || event.code),
        shortcut,
        ctrlKey: event.ctrlKey,
        shiftKey: event.shiftKey,
        altKey: event.altKey,
        metaKey: event.metaKey,
        source: "window",
        receivedAt: Date.now(),
        sequence: inputSnapshotRef.current.sequence + 1,
        down,
      });
    }

    function handleKeyUp(event) {
      if (generalKeyListening || isInteractiveDragTarget(event.target)) return;
      const shortcut = canonicalShortcut(eventShortcutFromKeyboardEvent(event));
      if (!shortcut) return;
      const down = new Set(inputSnapshotRef.current.down);
      down.delete(shortcut);
      pushInputEvent({
        id: `${Date.now()}-${inputSnapshotRef.current.sequence + 1}`,
        type: "keyup",
        key: normalizeInputKeyName(event.key || event.code),
        shortcut,
        ctrlKey: event.ctrlKey,
        shiftKey: event.shiftKey,
        altKey: event.altKey,
        metaKey: event.metaKey,
        source: "window",
        receivedAt: Date.now(),
        sequence: inputSnapshotRef.current.sequence + 1,
        down,
      });
    }

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
    };
  }, [generalKeyListening]);

  useEffect(() => {
    if (!controlsOpen) return undefined;
    setOpenConfigHint(null);
    if (openConfigHintTimeoutRef.current) {
      window.clearTimeout(openConfigHintTimeoutRef.current);
      openConfigHintTimeoutRef.current = null;
    }
    let disposed = false;
    async function refreshDebug() {
      try {
        const status = await invoke("get_game_overlay_debug_status");
        if (!disposed) setOverlayDebug(status);
      } catch (error) {
        if (!disposed) setOverlayDebug({ lastError: String(error) });
      }
    }
    refreshDebug();
    const interval = window.setInterval(refreshDebug, 1000);
    return () => {
      disposed = true;
      window.clearInterval(interval);
    };
  }, [controlsOpen]);

  useEffect(() => {
    if (config.general?.use_stream_overlays_api !== true) {
      setStreamOverlaysPayload(null);
      setStreamOverlaysAt(null);
      pushOverlayLog("info", "StreamOverlays API disabled");
    }
    return undefined;
  }, [config.general?.use_stream_overlays_api]);

  useEffect(() => {
    let unlistenControls = null;
    let unlistenConfig = null;
    let unlistenEndSummary = null;
    let unlistenLcStats = null;
    let unlistenStreamOverlays = null;
    let unlistenStreamOverlaysLog = null;
    let unlistenInput = null;
    let unlistenOpenConfigHint = null;
    let unlistenActive = null;
    let disposed = false;

    function pushTauriInputEvent(payload) {
      const shortcut = canonicalShortcut(payload?.shortcut ?? payload?.key ?? "");
      if (!shortcut) return;
      const type = payload?.state === "Released" || payload?.state === "released" || payload?.type === "keyup"
        ? "keyup"
        : "keydown";
      const parts = shortcutParts(shortcut);
      const down = new Set(inputSnapshotRef.current.down);
      if (type === "keydown") {
        down.add(shortcut);
      } else {
        down.delete(shortcut);
      }
      const event = {
        id: payload?.id ?? `${Date.now()}-${inputSnapshotRef.current.sequence + 1}`,
        type,
        key: parts.find((part) => !["Ctrl", "Shift", "Alt", "Meta"].includes(part)) ?? shortcut,
        shortcut,
        ctrlKey: parts.includes("Ctrl"),
        shiftKey: parts.includes("Shift"),
        altKey: parts.includes("Alt"),
        metaKey: parts.includes("Meta"),
        source: payload?.source ?? "global-shortcut",
        receivedAt: Date.now(),
        sequence: inputSnapshotRef.current.sequence + 1,
        down,
      };
      pushInputEvent(event);
    }

    (async () => {
      unlistenControls = await listen("overlay://controls-open-changed", (event) => {
        if (disposed) return;
        const open = !!event.payload;
        if (captureOnly) return;
        setControlsOpen(open);
        if (!open) setEditMode(false);
      });
      unlistenActive = await listen("overlay://active-changed", (event) => {
        if (disposed) return;
        const active = !!event.payload;
        setOverlayActive(active);
        if (!active) {
          setControlsOpen(false);
          setEditMode(false);
          setOpenConfigHint(null);
          inputSnapshotRef.current = createInputSnapshot();
          setInputSequence((current) => current + 1);
        }
      });
      unlistenConfig = await listen("overlay://config-changed", (event) => {
        if (!disposed) setConfig(normalizeConfig(event.payload, modules));
      });
      unlistenLcStats = await listen("overlay://lcstats-updated", (event) => {
        if (disposed) return;
        const payload = event.payload ?? null;
        const receivedAt = Date.now();
        setLcStatsPayload(payload);
        setLcStatsAt(receivedAt);
        setOverlayEvents((current) => [
          { type: "lcstats", payload, receivedAt },
          ...current,
        ].slice(0, 20));
      });
      unlistenStreamOverlays = await listen("overlay://stream-overlays-updated", (event) => {
        if (disposed) return;
        const payload = extractStreamOverlaysPayload(event.payload) ?? event.payload ?? null;
        if (!payload) return;
        setStreamOverlaysPayload(payload);
        setStreamOverlaysAt(Date.now());
      });
      unlistenStreamOverlaysLog = await listen("overlay://stream-overlays-log", (event) => {
        if (disposed) return;
        pushOverlayLog("warn", "StreamOverlays Rust monitor", event.payload ?? "");
      });
      unlistenInput = await listen("overlay://input-shortcut", (event) => {
        if (disposed) return;
        pushTauriInputEvent(event.payload ?? {});
      });
      unlistenOpenConfigHint = await listen("overlay://open-config-hint", (event) => {
        if (disposed || captureOnly || controlsOpen) return;
        showOpenConfigHint(event.payload ?? {});
      });
      unlistenEndSummary = await listen("overlay://show-end-summary", (event) => {
        if (disposed) return;
        const payload = event.payload ?? {};
        const duration = Number(payload.duration_ms ?? latestConfigRef.current?.general?.end_summary_duration_ms ?? 10000);
        setEndSummary({
          id: Date.now(),
          title: String(payload.title ?? "Run Summary"),
          lines: Array.isArray(payload.lines) ? payload.lines.map(String) : [],
          payload,
          expiresAt: Date.now() + Math.max(2000, Math.min(30000, duration)),
        });
      });
    })().catch(console.error);

    return () => {
      disposed = true;
      if (typeof unlistenControls === "function") unlistenControls();
      if (typeof unlistenConfig === "function") unlistenConfig();
      if (typeof unlistenEndSummary === "function") unlistenEndSummary();
      if (typeof unlistenLcStats === "function") unlistenLcStats();
      if (typeof unlistenStreamOverlays === "function") unlistenStreamOverlays();
      if (typeof unlistenStreamOverlaysLog === "function") unlistenStreamOverlaysLog();
      if (typeof unlistenInput === "function") unlistenInput();
      if (typeof unlistenOpenConfigHint === "function") unlistenOpenConfigHint();
      if (typeof unlistenActive === "function") unlistenActive();
    };
  }, [captureOnly, modules, controlsOpen]);

  useEffect(() => {
    if (captureOnly) return undefined;
    const shortcuts = collectModuleInputShortcuts(modules, config);
    invoke("set_game_overlay_input_shortcuts", { shortcuts }).catch((error) => {
      invoke("report_game_overlay_frontend_error", {
        message: `failed to register input shortcuts: ${error?.message ?? error}`,
      }).catch(console.error);
    });
  }, [captureOnly, modules, config.module_settings, config.general?.overlay_key]);

  useEffect(() => {
    if (!endSummary?.expiresAt) return undefined;
    const timeout = window.setTimeout(() => {
      setEndSummary((current) => (current?.id === endSummary.id ? null : current));
    }, Math.max(0, endSummary.expiresAt - Date.now()));
    return () => window.clearTimeout(timeout);
  }, [endSummary]);

  useEffect(() => {
    const stats = lcStatsPayload?.stats;
    if (!stats) {
      setLeaderboard({
        status: "idle",
        collections: HQ_LEADERBOARD_COLLECTIONS,
        boardTypes: HQ_LEADERBOARD_BOARD_TYPES,
        runFields: HQ_LEADERBOARD_RUN_FIELDS,
      });
      return undefined;
    }

    let cancelled = false;
    async function loadLeaderboardCheck() {
      const statsView = createLcStatsView(stats);
      const checkerSettings = latestConfigRef.current?.module_settings?.leaderboard
        ?? latestConfigRef.current?.module_settings?.record_checker
        ?? {};
      const input = getLeaderboardInput(statsView, stats, checkerSettings);
      if (!input.score) {
        setLeaderboard({
          status: "waiting",
          reason: "No comparable score in LCStats payload.",
          collections: HQ_LEADERBOARD_COLLECTIONS,
          boardTypes: HQ_LEADERBOARD_BOARD_TYPES,
          runFields: HQ_LEADERBOARD_RUN_FIELDS,
        });
        return;
      }
      setLeaderboard({
        status: "loading",
        ...input,
        collections: HQ_LEADERBOARD_COLLECTIONS,
        boardTypes: HQ_LEADERBOARD_BOARD_TYPES,
        runFields: HQ_LEADERBOARD_RUN_FIELDS,
      });
      try {
        const runs = await fetchHqRuns(input.collectionName);
        if (!cancelled) setLeaderboard(calculateLeaderboardCheck(runs, input, checkerSettings));
      } catch (error) {
        if (!cancelled) {
          setLeaderboard({
            status: "error",
            ...input,
            collections: HQ_LEADERBOARD_COLLECTIONS,
            boardTypes: HQ_LEADERBOARD_BOARD_TYPES,
            runFields: HQ_LEADERBOARD_RUN_FIELDS,
            error: error?.message ?? String(error),
          });
        }
        invoke("report_game_overlay_frontend_error", {
          message: `record checker failed: ${error?.message ?? error}`,
        }).catch(console.error);
      }
    }

    loadLeaderboardCheck();
    return () => {
      cancelled = true;
    };
  }, [lcStatsPayload, config.module_settings?.leaderboard, config.module_settings?.record_checker]);

  useEffect(() => {
    if (captureOnly) return undefined;
    function handleKeyDown(event) {
      if (event.key !== "Escape") return;
      if (editMode) {
        setEditMode(false);
        event.preventDefault();
        return;
      }
      if (controlsOpen) {
        closeControls();
        event.preventDefault();
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [captureOnly, controlsOpen, editMode]);

  useEffect(() => {
    function handleMove(event) {
      const controlsDrag = controlsDragRef.current;
      if (controlsDrag) {
        const width = controlsDrag.width;
        const height = controlsDrag.height;
        const left = Math.max(0, Math.min(window.innerWidth - width, event.clientX - controlsDrag.offsetX));
        const top = Math.max(0, Math.min(window.innerHeight - height, event.clientY - controlsDrag.offsetY));
        setControlsPosition({
          right: Math.max(0, window.innerWidth - left - width),
          top,
        });
        return;
      }

      const drag = dragRef.current;
      if (!drag) return;
      const x = ((event.clientX - drag.offsetX) / window.innerWidth) * 100;
      const y = ((event.clientY - drag.offsetY) / window.innerHeight) * 100;
      const widgetElements = Array.from(document.querySelectorAll("[data-overlay-widget]"));
      const dragElement = widgetElements.find((element) => element.dataset.overlayWidget === drag.id);
      const dragRect = dragElement?.getBoundingClientRect();
      const otherRects = widgetElements
        .filter((element) => element.dataset.overlayWidget !== drag.id)
        .map((element) => {
          const rect = element.getBoundingClientRect();
          return {
            id: element.dataset.overlayWidget,
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
            width: rect.width,
            height: rect.height,
          };
        });
      setConfig((current) => ({
        ...current,
        widgets: (() => {
          const currentWidget = current.widgets?.[drag.id];
          const { position, guides } = snapOverlayPosition({ x, y }, drag.id, current.widgets, currentWidget?.snap !== false, {
            dragRect,
            otherRects,
            viewportWidth: window.innerWidth,
            viewportHeight: window.innerHeight,
          });
          const nextWidgets = {
            ...current.widgets,
            [drag.id]: position,
          };
          setSnapGuides(guides);
          latestConfigRef.current = {
            ...current,
            widgets: nextWidgets,
          };
          return nextWidgets;
        })(),
      }));
    }

    function handleUp() {
      if (controlsDragRef.current) {
        controlsDragRef.current = null;
        return;
      }
      if (!dragRef.current) return;
      dragRef.current = null;
      setSnapGuides([]);
      saveConfig(latestConfigRef.current);
    }

    window.addEventListener("pointermove", handleMove);
    window.addEventListener("pointerup", handleUp);
    return () => {
      window.removeEventListener("pointermove", handleMove);
      window.removeEventListener("pointerup", handleUp);
    };
  }, []);

  function saveConfig(nextConfig) {
    const normalized = normalizeConfig(nextConfig, modules);
    const version = configVersionRef.current + 1;
    configVersionRef.current = version;
    latestConfigRef.current = normalized;
    setConfig(normalized);
    invoke("set_game_overlay_config", { config: normalized })
      .then((saved) => {
        if (configVersionRef.current !== version) return;
        const savedConfig = normalizeConfig(saved, modules);
        latestConfigRef.current = savedConfig;
        setConfig(savedConfig);
      })
      .catch(console.error);
  }

  function previewConfig(nextConfig) {
    const normalized = normalizeConfig(nextConfig, modules);
    configVersionRef.current += 1;
    latestConfigRef.current = normalized;
    setConfig(normalized);
  }

  function startDrag(event, id, moduleId = id) {
    if (!editMode) return;
    const rect = event.currentTarget.closest("[data-overlay-widget]")?.getBoundingClientRect();
    setSelectedModuleId(moduleId);
    dragRef.current = {
      id,
      offsetX: rect ? event.clientX - rect.left : 0,
      offsetY: rect ? event.clientY - rect.top : 0,
    };
    event.currentTarget.setPointerCapture?.(event.pointerId);
    event.preventDefault();
  }

  function startControlsDrag(event) {
    if (isInteractiveDragTarget(event.target)) return;
    const rect = event.currentTarget.closest("[data-overlay-controls]")?.getBoundingClientRect();
    if (!rect) return;
    controlsDragRef.current = {
      offsetX: event.clientX - rect.left,
      offsetY: event.clientY - rect.top,
      width: rect.width,
      height: rect.height,
    };
    event.currentTarget.setPointerCapture?.(event.pointerId);
    event.preventDefault();
  }

  function updateModuleSetting(moduleId, key, value, { persist = true } = {}) {
    const baseConfig = latestConfigRef.current;
    const nextConfig = {
      ...baseConfig,
      module_settings: {
        ...baseConfig.module_settings,
        [moduleId]: {
          ...(baseConfig.module_settings[moduleId] ?? {}),
          [key]: value,
        },
      },
    };
    if (persist) {
      saveConfig(nextConfig);
    } else {
      previewConfig(nextConfig);
    }
  }

  function updateGeneralSetting(key, value, { persist = true } = {}) {
    const baseConfig = latestConfigRef.current;
    const nextConfig = {
      ...baseConfig,
      general: {
        ...(baseConfig.general ?? DEFAULT_CONFIG.general),
        [key]: value,
      },
    };
    if (persist) {
      saveConfig(nextConfig);
    } else {
      previewConfig(nextConfig);
    }
  }

  function resetGeneralSetting(key) {
    updateGeneralSetting(key, DEFAULT_CONFIG.general[key]);
  }

  function updateWidgetSetting(moduleId, key, value) {
    saveConfig({
      ...config,
      widgets: {
        ...config.widgets,
        [moduleId]: {
          ...normalizePosition(config.widgets[moduleId], modules.find((module) => module.id === moduleId)?.defaultPosition),
          snap: config.widgets[moduleId]?.snap !== false,
          [key]: value,
        },
      },
    });
  }

  function resetWidgetPosition(module) {
    if (!module) return;
    saveConfig({
      ...config,
      widgets: {
        ...config.widgets,
        [module.id]: {
          ...normalizePosition(module.defaultPosition),
          snap: module.locked ? false : true,
        },
      },
    });
  }

  function closeControls() {
    invoke("set_game_overlay_controls_open", { open: false })
      .then((open) => setControlsOpen(!!open))
      .catch(console.error);
  }

  function reloadModules() {
    invoke("get_game_overlay_modules")
      .then((nextModules) => {
        const loaded = Array.isArray(nextModules) ? nextModules : [];
        const evaluated = loaded.flatMap(evaluateModule);
        setRawModules(loaded);
        setModuleLoadError(evaluated.length === 0 ? "No overlay modules loaded. Built-in fallback is active." : "");
      })
      .catch((error) => {
        console.error(error);
        setModuleLoadError(String(error));
        setRawModules([]);
      });
  }

  function openModulesFolder() {
    invoke("open_game_overlay_modules_folder").catch((error) => {
      console.error(error);
      pushOverlayLog("error", "Failed to open overlay module folder", String(error));
    });
  }

  const selectedModule = selectedModuleId === "general" || selectedModuleId === "logs"
    ? null
    : modules.find((module) => module.id === selectedModuleId) ?? modules[0] ?? null;
  const now = Date.now();
  const lcstatsPayload = lcStatsPayload;
  const lcstatsView = createLcStatsView(lcstatsPayload?.stats);
  const streamOverlays = streamOverlaysPayload;
  const context = {
    editMode,
    controlsOpen,
    elapsedSeconds,
    lcstats: lcstatsView,
    lcstatsRaw: lcStatsPayload?.raw ?? null,
    lcstatsPayload,
    lcstatsAgeMs: lcStatsAt ? now - lcStatsAt : null,
    streamOverlays,
    streamOverlay: streamOverlays,
    streamOverlaysAgeMs: streamOverlaysAt ? now - streamOverlaysAt : null,
    displayTimeMs: Number(config.general?.end_summary_duration_ms ?? DEFAULT_CONFIG.general.end_summary_duration_ms),
    leaderboard,
    recordChecker: leaderboard,
    endSummary,
    events: overlayEvents,
    inputSequence,
    formatSeconds,
    escapeHtml,
    html: escapeHtml,
    number,
    stripLcQuote,
    intish,
    valueAt,
    valueAtAny,
  };
  overlayContextRef.current = context;

  const renderedModules = modules
    .flatMap((module) => {
      const settings = config.module_settings[module.id] ?? module.defaultSettings;
      const api = createModuleApi(module.id, inputSnapshotRef, consumedInputRef, overlayContextRef);
      let visible = false;
      let rendered = "";
      let data = context;
      try {
        module.tick?.({ context, settings, config, api });
        data = module.derive({ context, settings, config, api }) ?? context;
        visible = !!module.visible({ context, data, settings, config, api });
        rendered = module.render({ context, data, settings, config, api }) ?? "";
      } catch (error) {
        visible = editMode;
        rendered = `<div class="overlay-title">${escapeHtml(module.name)}</div><div class="overlay-line">Module error</div>`;
        console.error(`Overlay module ${module.id} failed`, error);
        pushOverlayLog("error", `Module ${module.id} failed`, String(error?.message ?? error));
        invoke("report_game_overlay_frontend_error", {
          message: `module ${module.id} render failed: ${error?.message ?? error}`,
        }).catch(console.error);
      }
      if (!visible) return [];
      return overlayRenderEntries(module, rendered).map((entry) => ({ ...entry, settings }));
    })
    .filter(Boolean);

  const renderReport = [
    `modules=${modules.length}`,
    `moduleIds=${modules.map((module) => module.id).join(",") || "none"}`,
    `rendered=${renderedModules.length}`,
    `renderedIds=${renderedModules.map((entry) => entry.widgetId).join(",") || "none"}`,
    `hiddenIds=${modules
      .filter((module) => !renderedModules.some((entry) => entry.module.id === module.id))
      .map((module) => `${module.id}:enabled=${String((config.module_settings[module.id] ?? module.defaultSettings)?.enabled)}`)
      .join(",") || "none"}`,
    `controlsOpen=${controlsOpen}`,
    `editMode=${editMode}`,
  ].join(" ");

  useEffect(() => {
    if (renderReportRef.current === renderReport) return;
    renderReportRef.current = renderReport;
    invoke("report_game_overlay_frontend_info", { message: `render ${renderReport}` }).catch(console.error);
  }, [renderReport]);

  const moduleCss = modules
    .filter((module) => module.css.trim())
    .map((module) => `/* ${module.fileName} */\n${module.css}`)
    .join("\n\n");

  return (
    <div
      className={cn(
        "relative h-screen w-screen overflow-hidden bg-transparent text-white",
        overlayActive && controlsOpen ? "pointer-events-auto" : "pointer-events-none",
      )}
    >
      <style>{moduleCss}</style>

      {overlayActive && editMode
        ? snapGuides.map((guide, index) => (
            <div
              key={`${guide.axis}-${guide.value}-${index}`}
              className={cn(
                "pointer-events-none fixed z-[2147482999] bg-[var(--theme-accent)]/75 shadow-[0_0_12px_rgba(255,255,255,0.35)]",
                guide.axis === "x" ? "top-0 h-screen w-px" : "left-0 h-px w-screen",
              )}
              style={guide.axis === "x" ? { left: `${guide.value}px` } : { top: `${guide.value}px` }}
            />
          ))
        : null}

      {overlayActive
        ? renderedModules.map((entry) => (
            <OverlayModuleView
              key={entry.renderId}
              entry={entry}
              position={normalizePosition(config.widgets[entry.widgetId], entry.defaultPosition)}
              editMode={editMode}
              onDragStart={startDrag}
            />
          ))
        : null}

      {overlayActive && editMode ? (
        <div className="pointer-events-none fixed inset-0 border-2 border-[var(--theme-accent)]/55 bg-[var(--theme-accent)]/5" />
      ) : null}

      {overlayActive && !captureOnly && openConfigHint && !controlsOpen ? (
        <div className="pointer-events-none fixed left-1/2 top-8 z-[2147483000] -translate-x-1/2">
          <div className="flex max-w-[min(420px,calc(100vw-2rem))] items-center gap-3 rounded border border-white/15 bg-[#121318]/95 px-4 py-3 text-sm text-white shadow-2xl shadow-black/50 backdrop-blur">
            <Keyboard className="h-4 w-4 shrink-0 text-[var(--theme-accent)]" />
            <div className="min-w-0">
              <span className="text-white/75">Press </span>
              <span className="rounded border border-white/15 bg-white/10 px-2 py-0.5 font-mono text-xs text-white">
                {openConfigHint.shortcut}
              </span>
              <span className="text-white/75"> to open overlay settings</span>
            </div>
          </div>
        </div>
      ) : null}

      {overlayActive && controlsOpen ? (
        <div className="pointer-events-none fixed inset-0 bg-black/35">
          <div
            data-overlay-controls
            className="pointer-events-auto absolute flex h-[min(560px,calc(100vh-3rem))] w-[min(760px,calc(100vw-3rem))] overflow-hidden rounded border border-white/15 bg-[#121318]/98 text-white shadow-2xl shadow-black/60 backdrop-blur"
            style={{ right: `${controlsPosition.right}px`, top: `${controlsPosition.top}px` }}
          >
            <div className="flex w-60 shrink-0 flex-col border-r border-white/10 bg-black/25">
              <div
                className="flex h-12 cursor-move select-none items-center justify-between border-b border-white/10 px-3"
                onPointerDown={startControlsDrag}
              >
                <div className="min-w-0">
                  <div className="truncate text-sm font-semibold">Overlay Modules</div>
                  <div className="truncate text-[11px] text-white/40">asta.hq-launcher/overlayModule</div>
                </div>
                <button
                  type="button"
                  className="flex h-8 w-8 items-center justify-center rounded text-white/55 hover:bg-white/10 hover:text-white"
                  onClick={closeControls}
                  aria-label="Close overlay settings"
                >
                  <X className="h-4 w-4" />
                </button>
              </div>
              <div className="min-h-0 flex-1 overflow-auto p-2">
                <button
                  type="button"
                  className={cn(
                    "mb-1 flex w-full items-center justify-between rounded px-3 py-2 text-left text-sm transition",
                    selectedModuleId === "general"
                      ? "bg-[var(--theme-accent)] text-black"
                      : "text-white/75 hover:bg-white/10 hover:text-white",
                  )}
                  onClick={() => setSelectedModuleId("general")}
                >
                  <span className="min-w-0 truncate">General</span>
                </button>
                <button
                  type="button"
                  className={cn(
                    "mb-1 flex w-full items-center justify-between rounded px-3 py-2 text-left text-sm transition",
                    selectedModuleId === "logs"
                      ? "bg-[var(--theme-accent)] text-black"
                      : "text-white/75 hover:bg-white/10 hover:text-white",
                  )}
                  onClick={() => setSelectedModuleId("logs")}
                >
                  <span className="min-w-0 truncate">Logs</span>
                  <span className="text-[10px] opacity-60">{overlayLogs.length}</span>
                </button>
                {modules.map((module) => (
                  <button
                    key={module.id}
                    type="button"
                    className={cn(
                      "mb-1 flex w-full items-center justify-between rounded px-3 py-2 text-left text-sm transition",
                      selectedModule?.id === module.id
                        ? "bg-[var(--theme-accent)] text-black"
                        : "text-white/75 hover:bg-white/10 hover:text-white",
                    )}
                    onClick={() => setSelectedModuleId(module.id)}
                  >
                    <span className="min-w-0 truncate">{module.name}</span>
                    {module.locked ? <span className="text-[10px] opacity-60">LOCK</span> : null}
                  </button>
                ))}
              </div>
              <div className="border-t border-white/10 p-2">
                <div className="grid grid-cols-2 gap-2">
                  <Button variant="secondary" size="sm" className="h-9 justify-center rounded" onClick={openModulesFolder}>
                    <FolderOpen className="h-4 w-4" />
                    Folder
                  </Button>
                  <Button variant="secondary" size="sm" className="h-9 justify-center rounded" onClick={reloadModules}>
                    <RefreshCw className="h-4 w-4" />
                    Reload
                  </Button>
                </div>
              </div>
            </div>

            <div className="flex min-w-0 flex-1 flex-col">
              <div
                className="flex h-12 cursor-move select-none items-center justify-between border-b border-white/10 px-4"
                onPointerDown={startControlsDrag}
              >
                <div className="min-w-0">
                  <div className="truncate text-sm font-semibold">
                    {selectedModuleId === "general" ? "General" : selectedModuleId === "logs" ? "Logs" : selectedModule?.name ?? "No Module"}
                  </div>
                  <div className="truncate text-[11px] text-white/40">
                    {selectedModuleId === "general" ? "Overlay defaults" : selectedModuleId === "logs" ? "Overlay diagnostics" : selectedModule?.fileName ?? ""}
                  </div>
                </div>
                <Button
                  variant="secondary"
                  size="sm"
                  className={cn("h-9 rounded px-3", editMode ? "bg-white/10" : "bg-black/20")}
                  onClick={() => setEditMode((current) => !current)}
                >
                  <Pencil className="h-4 w-4" />
                  {editMode ? "Done" : "Edit Layout"}
                </Button>
              </div>
              <div className="min-h-0 flex-1 overflow-auto p-4">
                {moduleLoadError ? (
                  <div className="mb-4 rounded border border-yellow-300/25 bg-yellow-300/10 px-3 py-2 text-xs leading-relaxed text-yellow-100">
                    {moduleLoadError}
                  </div>
                ) : null}
                {selectedModuleId === "general" ? (
                  <div className="space-y-4">
                    <div>
                      <div className="mb-2 flex items-center justify-between gap-3 text-xs text-white/55">
                        <span>Open Overlay Key</span>
                        <span className="flex items-center gap-2">
                          {generalKeyListening ? <span className="text-[var(--theme-accent)]">Listening...</span> : null}
                          <ResetButton onClick={() => resetGeneralSetting("overlay_key")} />
                        </span>
                      </div>
                      <KeyCaptureButton
                        active={generalKeyListening}
                        value={config.general?.overlay_key ?? "Insert"}
                        onStart={() => setGeneralKeyListening(true)}
                        onCancel={() => setGeneralKeyListening(false)}
                        onCapture={(nextKey) => {
                          updateGeneralSetting("overlay_key", nextKey);
                          setGeneralKeyListening(false);
                        }}
                      />
                    </div>
                    <div>
                      <div className="mb-2 flex items-center justify-between gap-3 text-xs text-white/55">
                        <span>Display Time</span>
                        <span className="flex items-center gap-2">
                          <span className="tabular-nums text-white/40">{Math.round(Number(config.general?.end_summary_duration_ms ?? 10000) / 1000)}s</span>
                          <ResetButton onClick={() => resetGeneralSetting("end_summary_duration_ms")} />
                        </span>
                      </div>
                      <Slider
                        value={[Number(config.general?.end_summary_duration_ms ?? 10000) / 1000]}
                        min={2}
                        max={30}
                        step={1}
                        onValueChange={([next]) => updateGeneralSetting("end_summary_duration_ms", next * 1000, { persist: false })}
                        onValueCommit={([next]) => updateGeneralSetting("end_summary_duration_ms", next * 1000)}
                      />
                    </div>
                  </div>
                ) : selectedModuleId === "logs" ? (
                  <div className="space-y-3">
                    <div className="flex items-center justify-between gap-3">
                      <div>
                        <div className="text-sm font-medium text-white/85">Overlay Logs</div>
                        <div className="text-xs text-white/40">
                          StreamOverlays connection and module diagnostics.
                        </div>
                      </div>
                      <Button
                        variant="secondary"
                        size="sm"
                        className="h-8 rounded px-3"
                        onClick={() => setOverlayLogs([])}
                      >
                        Clear
                      </Button>
                    </div>
                    <div className="rounded border border-white/10 bg-black/25">
                      {overlayLogs.length === 0 ? (
                        <div className="px-3 py-6 text-center text-xs text-white/40">No logs yet.</div>
                      ) : (
                        <div className="max-h-[420px] overflow-auto">
                          {overlayLogs.map((log) => {
                            const color = log.level === "error"
                              ? "text-red-200"
                              : log.level === "warn"
                                ? "text-yellow-100"
                                : log.level === "success"
                                  ? "text-emerald-100"
                                  : "text-white/75";
                            return (
                              <div key={log.id} className="border-b border-white/10 px-3 py-2 last:border-b-0">
                                <div className="flex items-center justify-between gap-3">
                                  <span className={cn("truncate text-xs font-medium", color)}>{log.message}</span>
                                  <span className="shrink-0 text-[10px] tabular-nums text-white/35">
                                    {new Date(log.at).toLocaleTimeString()}
                                  </span>
                                </div>
                                {log.details !== undefined && log.details !== "" ? (
                                  <div className="mt-1 break-all font-mono text-[11px] leading-relaxed text-white/45">
                                    {String(log.details)}
                                  </div>
                                ) : null}
                              </div>
                            );
                          })}
                        </div>
                      )}
                    </div>
                  </div>
                ) : selectedModule?.description ? (
                  <div className="mb-4 rounded border border-white/10 bg-black/20 px-3 py-2 text-xs leading-relaxed text-white/55">
                    {selectedModule.description}
                  </div>
                ) : null}
                {selectedModule ? (
                  <>
                    <div className="mb-4 rounded border border-white/10 bg-black/20 px-3 py-3">
                      <div className="mb-3 flex items-center justify-between gap-3">
                        <div>
                          <div className="text-sm font-medium text-white/80">Layout</div>
                          <div className="text-xs text-white/40">Drag the overlay itself while Edit Layout is enabled.</div>
                        </div>
                        <ResetButton onClick={() => resetWidgetPosition(selectedModule)} />
                      </div>
                      <label className="flex items-center justify-between gap-3 rounded border border-white/10 bg-black/20 px-3 py-2">
                        <span className="text-sm text-white/80">Snap</span>
                        <input
                          type="checkbox"
                          checked={config.widgets[selectedModule.id]?.snap !== false}
                          disabled={selectedModule.locked}
                          onChange={(event) => updateWidgetSetting(selectedModule.id, "snap", event.target.checked)}
                          className="h-4 w-4 accent-[var(--theme-accent)] disabled:opacity-35"
                        />
                      </label>
                    </div>
                    <ModuleSettings
                      module={selectedModule}
                      settings={selectedModule ? config.module_settings[selectedModule.id] ?? {} : {}}
                      onChange={(key, value) => updateModuleSetting(selectedModule.id, key, value)}
                      onPreview={(key, value) => updateModuleSetting(selectedModule.id, key, value, { persist: false })}
                      onReset={(item) => updateModuleSetting(selectedModule.id, item.key, resetValueForSetting(item))}
                    />
                  </>
                ) : null}
              </div>
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}
