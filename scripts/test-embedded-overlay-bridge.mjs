import assert from "node:assert/strict";

const modules = new Map([
  ["crosshair.js", "setName('Crosshair');"],
  ["game_timer.js", "setName('Real Time');"],
]);
const configFiles = new Map([
  ["general.json", JSON.stringify({ enabled: true, backend: "native", overlay_key: "PageDown" })],
  ["widgets.json", JSON.stringify({ crosshair: { x: 50, y: 50 } })],
  ["modules/crosshair.json", JSON.stringify({ enabled: true, style: "dot" })],
  ["modules/game_timer.json", "{ definitely not json }"],
]);
const requests = [];
let messageHandler = null;
let frontendReady = false;

function responseFor(command, payload) {
  if (command === "module.list") return [...modules.keys()].join("\n");
  if (command === "module.read") return modules.get(payload) ?? "";
  if (command === "config.list") return [...configFiles.keys()].join("\n");
  if (command === "config.read") return configFiles.get(payload) ?? "";
  if (command === "config.write") {
    const newline = payload.indexOf("\n");
    configFiles.set(payload.slice(0, newline), payload.slice(newline + 1));
    return "";
  }
  if (command === "ui.controls" || command === "dialog.active") return payload;
  if (command === "ui.shortcuts" || command === "folder.open") return "true";
  if (command === "debug.status") return JSON.stringify({ controlsOpen: false, effectiveBackend: "native" });
  if (command === "lcstats.latest") return "";
  if (command === "frontend.ready") {
    frontendReady = true;
    return "";
  }
  if (command === "frontend.info" || command === "frontend.error") return "";
  throw new Error(`Unexpected host command: ${command}`);
}

const webview = {
  addEventListener(name, handler) {
    assert.equal(name, "message");
    messageHandler = handler;
  },
  postMessage(message) {
    assert.equal(typeof message, "string");
    const newline = message.indexOf("\n");
    const [version, id, command] = message.slice(0, newline).split("\t");
    const payload = message.slice(newline + 1);
    assert.equal(version, "HQ1");
    requests.push({ id, command, payload });
    queueMicrotask(() => {
      try {
        const result = responseFor(command, payload);
        messageHandler({ data: `HQ1R\t${id}\tOK\n${result}` });
      } catch (error) {
        messageHandler({ data: `HQ1R\t${id}\tERR\n${error.message}` });
      }
    });
  },
};

globalThis.window = {
  __HQ_EMBEDDED_OVERLAY__: true,
  chrome: { webview },
  setTimeout,
  clearTimeout,
  queueMicrotask,
};

const bridge = await import("../src/lib/gameOverlayBridge.js");

const loadedModules = await bridge.invoke("get_game_overlay_modules");
assert.deepEqual(loadedModules, [
  { id: "crosshair", file_name: "crosshair.js", source: "setName('Crosshair');" },
  { id: "game_timer", file_name: "game_timer.js", source: "setName('Real Time');" },
]);

const loadedConfig = await bridge.invoke("get_game_overlay_config");
assert.equal(loadedConfig.general.overlay_key, "PageDown");
assert.equal(loadedConfig.module_settings.crosshair.style, "dot");
assert.deepEqual(loadedConfig.module_settings.game_timer, {});
assert(requests.some((request) => request.command === "frontend.error"
  && request.payload.includes("modules/game_timer.json")));

loadedConfig.module_settings.crosshair.style = "circle";
const savedConfig = await bridge.invoke("set_game_overlay_config", { config: loadedConfig });
assert.equal(savedConfig.module_settings.crosshair.style, "circle");
assert.equal(await bridge.invoke("set_game_overlay_controls_open", { open: true }), true);
assert.equal(await bridge.invoke("set_game_overlay_input_shortcuts", { shortcuts: ["X", "Ctrl+K"] }), true);

// Native can announce activity as soon as its DOM is ready, before React's
// listen effect has attached. The adapter must retain that event.
messageHandler({ data: "HQ1E\toverlay://active-changed\ntrue" });
const receivedEvent = new Promise((resolve) => {
  bridge.listen("overlay://active-changed", resolve);
});
assert.deepEqual(await receivedEvent, {
  event: "overlay://active-changed",
  id: 0,
  payload: true,
});

await bridge.invoke("report_game_overlay_frontend_ready");
assert.equal(frontendReady, true);
assert(requests.some((request) => request.command === "config.write"));

window.__HQ_EMBEDDED_OVERLAY__ = false;
window.__TAURI_INTERNALS__ = {
  invoke: async (command, args) => ({ command, args }),
};
assert.deepEqual(await bridge.invoke("legacy_regression_check", { value: 7 }), {
  command: "legacy_regression_check",
  args: { value: 7 },
});

console.log(`embedded overlay bridge ok (${requests.length} host requests)`);
