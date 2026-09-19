import { OVERLAY_ENEMY_CATALOG, normalizedEnemyCounts } from "./enemies";
import { createInputApi, createInputSnapshot } from "./input";
import { normalizePosition, normalizeWidgetPosition, safeClassName, safeOverlayId } from "./layout";
import { normalizedScrapSummary } from "./scrap";
import { escapeHtml, formatSeconds, intish, number, stripLcQuote, valueAt, valueAtAny } from "./values";

export const DEFAULT_CONFIG = {
  general: {
    enabled: true,
    backend: "native",
    backend_migration_version: 1,
    inject_all_processes: false,
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

export function overlayErrorMessage(error) {
  return String(error?.message ?? error ?? "Unknown error");
}

export function overlayErrorStack(error) {
  return String(error?.stack ?? error?.message ?? error ?? "Unknown error");
}

export function createDiagnosticModule(raw, error) {
  const id = safeOverlayId(`error_${raw?.id || raw?.file_name?.replace(/\.js$/i, "") || "module"}`);
  const fileName = raw?.file_name || raw?.id || "unknown module";
  const message = overlayErrorMessage(error);
  return {
    id,
    fileName,
    name: `Broken: ${fileName}`,
    description: `This overlay module failed to load: ${message}`,
    locked: true,
    defaultPosition: { x: 4, y: 12 },
    defaultSettings: {},
    settings: [],
    css: "",
    wrapperClass: "rounded border border-red-300/30 bg-red-950/70 px-3 py-2 text-xs text-red-100 shadow-xl shadow-black/50",
    visible: ({ context }) => !!context.editMode,
    derive: ({ context }) => context,
    tick: () => {},
    render: () => `<div class="font-semibold">Module load failed</div><div>${escapeHtml(fileName)}</div><div>${escapeHtml(message)}</div>`,
    loadError: {
      moduleId: id,
      moduleName: fileName,
      fileName,
      phase: "load",
      message,
      stack: overlayErrorStack(error),
    },
  };
}

export function isStreamOverlaysPayload(value) {
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

export function extractStreamOverlaysPayload(value) {
  if (!value || typeof value !== "object") return null;
  const candidates = [
    value,
    value.data,
    value.payload,
    value.message,
  ];
  return candidates.find(isStreamOverlaysPayload) ?? null;
}

export function createModuleApi(
  moduleId,
  inputSnapshotRef = { current: createInputSnapshot() },
  consumedInputRef = { current: new Set() },
  contextRef = { current: null },
) {
  const currentContext = () => contextRef.current ?? {};
  const scrapApi = {
    summary: () => normalizedScrapSummary(currentContext()),
    moon: () => normalizedScrapSummary(currentContext()).moon,
    remaining: () => normalizedScrapSummary(currentContext()).remaining,
    total: () => normalizedScrapSummary(currentContext()).total,
    items: () => normalizedScrapSummary(currentContext()).items,
    groups: () => normalizedScrapSummary(currentContext()).groups,
  };
  const enemiesApi = {
    list: () => OVERLAY_ENEMY_CATALOG.map((enemy) => ({ ...enemy, names: enemy.names.slice() })),
    counts: (enemy, options) => normalizedEnemyCounts(currentContext(), enemy, options),
    spawned: (enemy, options) => normalizedEnemyCounts(currentContext(), enemy, options).spawned,
    killed: (enemy, options) => normalizedEnemyCounts(currentContext(), enemy, options).killed,
    alive: (enemy, options) => normalizedEnemyCounts(currentContext(), enemy, options).alive,
    present: (enemy, options) => normalizedEnemyCounts(currentContext(), enemy, options).present,
    butler: (options) => normalizedEnemyCounts(currentContext(), "butler", options),
    nutcracker: (options) => normalizedEnemyCounts(currentContext(), "nutcracker", options),
  };

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
    scrap: scrapApi,
    enemies: enemiesApi,
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

export function normalizeSettingsSchema(items) {
  return (Array.isArray(items) ? items : [])
    .filter((item) => item && item.key)
    .map((item) => ({ ...item, key: String(item.key) }));
}

export function defaultSettingsFromSchema(items) {
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

export function createCtRuntime(raw) {
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

export function evaluateModule(raw) {
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
    return [createDiagnosticModule(raw, error)];
  }
}

export function normalizeConfig(config, modules) {
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

export function settingValue(settings, item) {
  if (settings[item.key] !== undefined) return settings[item.key];
  return resetValueForSetting(item);
}

export function resetValueForSetting(item) {
  if (item.default !== undefined) return item.default;
  if (item.type === "boolean") return false;
  if (item.type === "color") return "#ffffff";
  if (item.type === "select") return item.options?.[0]?.value ?? "";
  if (item.type === "number") return item.min ?? 0;
  if (item.type === "images") return [];
  if (item.type === "key" || item.type === "image") return "";
  return item.min ?? "";
}
