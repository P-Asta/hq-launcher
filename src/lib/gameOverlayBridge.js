import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen } from "@tauri-apps/api/event";

const DEFAULT_RPC_TIMEOUT_MS = 15_000;
const MAX_QUEUED_EVENTS_PER_NAME = 16;

let nextRequestId = 0;
let embeddedMessageBound = false;
const pendingRequests = new Map();
const embeddedListeners = new Map();
const queuedEvents = new Map();

function isEmbeddedOverlay() {
  return typeof window !== "undefined" && window.__HQ_EMBEDDED_OVERLAY__ === true;
}

function embeddedWebView() {
  const webview = window?.chrome?.webview;
  if (!webview || typeof webview.postMessage !== "function") {
    throw new Error("The embedded overlay WebView2 host bridge is unavailable.");
  }
  return webview;
}

function splitEnvelope(value) {
  if (typeof value !== "string") return null;
  const newline = value.indexOf("\n");
  return newline < 0
    ? { header: value, payload: "" }
    : { header: value.slice(0, newline), payload: value.slice(newline + 1) };
}

function rpcError(payload, command = "") {
  const message = payload || `Embedded overlay host request failed${command ? `: ${command}` : "."}`;
  return new Error(message);
}

function settleHostResponse(header, payload) {
  const [version, id, status] = header.split("\t");
  if (version !== "HQ1R" || !id) return false;
  const pending = pendingRequests.get(id);
  if (!pending) return true;

  pendingRequests.delete(id);
  window.clearTimeout(pending.timeoutId);
  if (status === "OK") {
    pending.resolve(payload);
  } else {
    pending.reject(rpcError(payload, pending.command));
  }
  return true;
}

function parseEventPayload(payload) {
  if (!payload) return null;
  try {
    return JSON.parse(payload);
  } catch {
    return payload;
  }
}

function deliverEmbeddedEvent(event) {
  const listeners = embeddedListeners.get(event.event);
  if (!listeners || listeners.size === 0) {
    const queued = queuedEvents.get(event.event) ?? [];
    queued.push(event);
    if (queued.length > MAX_QUEUED_EVENTS_PER_NAME) queued.shift();
    queuedEvents.set(event.event, queued);
    return;
  }

  for (const listener of [...listeners]) {
    try {
      listener(event);
    } catch (error) {
      window.setTimeout(() => {
        throw error;
      }, 0);
    }
  }
}

function handleEmbeddedHostMessage(event) {
  const envelope = splitEnvelope(event?.data);
  if (!envelope) return;
  if (settleHostResponse(envelope.header, envelope.payload)) return;

  const [version, eventName] = envelope.header.split("\t");
  if (version !== "HQ1E" || !eventName) return;
  deliverEmbeddedEvent({
    event: eventName,
    id: 0,
    payload: parseEventPayload(envelope.payload),
  });
}

function ensureEmbeddedMessageHandler() {
  if (embeddedMessageBound) return embeddedWebView();
  const webview = embeddedWebView();
  if (typeof webview.addEventListener !== "function") {
    throw new Error("The embedded overlay WebView2 host cannot receive messages.");
  }
  webview.addEventListener("message", handleEmbeddedHostMessage);
  embeddedMessageBound = true;
  return webview;
}

function hostInvoke(command, payload = "") {
  let webview;
  try {
    webview = ensureEmbeddedMessageHandler();
  } catch (error) {
    return Promise.reject(error);
  }

  const id = String(++nextRequestId);
  const timeoutMs = Math.max(
    1_000,
    Number(window.__HQ_EMBEDDED_OVERLAY_RPC_TIMEOUT_MS__ ?? DEFAULT_RPC_TIMEOUT_MS) || DEFAULT_RPC_TIMEOUT_MS,
  );

  return new Promise((resolve, reject) => {
    const timeoutId = window.setTimeout(() => {
      pendingRequests.delete(id);
      reject(new Error(`Embedded overlay host request timed out: ${command}`));
    }, timeoutMs);
    pendingRequests.set(id, { resolve, reject, timeoutId, command });

    try {
      webview.postMessage(`HQ1\t${id}\t${command}\n${String(payload ?? "")}`);
    } catch (error) {
      pendingRequests.delete(id);
      window.clearTimeout(timeoutId);
      reject(error);
    }
  });
}

function embeddedListen(eventName, handler) {
  ensureEmbeddedMessageHandler();
  const name = String(eventName);
  const listeners = embeddedListeners.get(name) ?? new Set();
  listeners.add(handler);
  embeddedListeners.set(name, listeners);

  const queued = queuedEvents.get(name);
  if (queued?.length) {
    queuedEvents.delete(name);
    window.queueMicrotask(() => {
      if (!listeners.has(handler)) return;
      for (const event of queued) handler(event);
    });
  }

  let listening = true;
  return Promise.resolve(() => {
    if (!listening) return;
    listening = false;
    listeners.delete(handler);
    if (listeners.size === 0) embeddedListeners.delete(name);
  });
}

function lines(payload) {
  return String(payload ?? "")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
}

function safeRelativePath(value) {
  const normalized = String(value ?? "").replaceAll("\\", "/").replace(/^\.\//, "");
  if (!normalized || normalized.startsWith("/") || normalized.includes("\0")) return "";
  if (normalized.split("/").some((part) => !part || part === "." || part === "..")) return "";
  return normalized;
}

function parseJson(payload, fallback) {
  if (!String(payload ?? "").trim()) return fallback;
  return JSON.parse(payload);
}

async function readEmbeddedConfigObject(relativePath) {
  try {
    const value = parseJson(await hostInvoke("config.read", relativePath), {});
    if (!value || typeof value !== "object" || Array.isArray(value)) {
      throw new Error("configuration root must be a JSON object");
    }
    return value;
  } catch (error) {
    const message = `Ignoring malformed native overlay config ${relativePath}: ${error?.message ?? error}`;
    await hostInvoke("frontend.error", message).catch(() => {});
    return {};
  }
}

function parseHostBoolean(payload, fallback) {
  const normalized = String(payload ?? "").trim().toLowerCase();
  if (normalized === "true" || normalized === "1" || normalized === "ok") return true;
  if (normalized === "false" || normalized === "0") return false;
  return fallback;
}

function moduleSettingsFileName(moduleId) {
  const stem = String(moduleId ?? "")
    .trim()
    .replace(/[^a-z0-9_-]/gi, "_")
    .replace(/^_+|_+$/g, "");
  return stem || "module";
}

async function getEmbeddedModules() {
  const listed = await hostInvoke("module.list");
  const files = lines(listed)
    .map(safeRelativePath)
    .filter((file) => file && !file.includes("/") && file.toLowerCase().endsWith(".js"));
  return Promise.all(files.map(async (fileName) => ({
    id: fileName.slice(0, -3).trim(),
    file_name: fileName,
    source: await hostInvoke("module.read", fileName),
  })));
}

async function getEmbeddedConfig() {
  const config = {
    general: {},
    crosshair: {},
    widgets: {},
    module_settings: {},
    end_summary: {},
  };
  const listed = await hostInvoke("config.list");
  const files = lines(listed).map(safeRelativePath).filter(Boolean);

  await Promise.all(files.map(async (relativePath) => {
    const value = await readEmbeddedConfigObject(relativePath);
    const lower = relativePath.toLowerCase();
    if (lower === "general.json") config.general = value;
    else if (lower === "crosshair.json") config.crosshair = value;
    else if (lower === "widgets.json") config.widgets = value;
    else if (lower === "end_summary.json") config.end_summary = value;
    else if (lower.startsWith("modules/") && lower.endsWith(".json")) {
      const moduleId = relativePath.slice("modules/".length, -".json".length);
      if (moduleId) config.module_settings[moduleId] = value;
    }
  }));
  return config;
}

async function writeEmbeddedConfig(config) {
  const writes = [
    ["general.json", config?.general ?? {}],
    ["crosshair.json", config?.crosshair ?? {}],
    ["widgets.json", config?.widgets ?? {}],
    ["end_summary.json", config?.end_summary ?? {}],
    ...Object.entries(config?.module_settings ?? {}).map(([moduleId, settings]) => [
      `modules/${moduleSettingsFileName(moduleId)}.json`,
      settings ?? {},
    ]),
  ];

  for (const [relativePath, value] of writes) {
    await hostInvoke("config.write", `${relativePath}\n${JSON.stringify(value, null, 2)}`);
  }
  return getEmbeddedConfig();
}

async function embeddedInvoke(command, args = {}) {
  switch (command) {
    case "get_game_overlay_modules":
      return getEmbeddedModules();
    case "get_game_overlay_config":
      return getEmbeddedConfig();
    case "set_game_overlay_config":
      return writeEmbeddedConfig(args.config ?? {});
    case "get_lcstats_latest_payload":
      return parseJson(await hostInvoke("lcstats.latest"), null);
    case "get_game_overlay_debug_status":
      return parseJson(await hostInvoke("debug.status"), {});
    case "set_game_overlay_controls_open":
      return parseHostBoolean(await hostInvoke("ui.controls", String(!!args.open)), !!args.open);
    case "set_game_overlay_input_shortcuts":
      return parseHostBoolean(
        await hostInvoke("ui.shortcuts", (Array.isArray(args.shortcuts) ? args.shortcuts : []).join("\n")),
        true,
      );
    case "set_game_overlay_file_dialog_active":
      return parseHostBoolean(await hostInvoke("dialog.active", String(!!args.active)), !!args.active);
    case "open_game_overlay_modules_folder":
      return parseHostBoolean(await hostInvoke("folder.open", "overlayModule"), true);
    case "report_game_overlay_frontend_ready":
      await hostInvoke("frontend.ready");
      return null;
    case "report_game_overlay_frontend_info":
      await hostInvoke("frontend.info", String(args.message ?? ""));
      return null;
    case "report_game_overlay_frontend_error":
      await hostInvoke("frontend.error", String(args.message ?? ""));
      return null;
    default:
      throw new Error(`Unsupported embedded overlay command: ${command}`);
  }
}

export function invoke(command, args = {}) {
  if (isEmbeddedOverlay()) return embeddedInvoke(command, args);
  return tauriInvoke(command, args);
}

export function listen(event, handler, options) {
  if (isEmbeddedOverlay()) return embeddedListen(event, handler);
  return tauriListen(event, handler, options);
}

export const embeddedOverlayProtocol = Object.freeze({
  requestPrefix: "HQ1",
  responsePrefix: "HQ1R",
  eventPrefix: "HQ1E",
});
