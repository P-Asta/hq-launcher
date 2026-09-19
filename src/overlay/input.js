export function normalizeInputKeyName(value) {
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

export function shortcutParts(value) {
  return String(value ?? "")
    .split("+")
    .map((part) => normalizeInputKeyName(part))
    .filter(Boolean);
}

export function canonicalShortcut(value) {
  const parts = shortcutParts(value);
  const modifiers = [];
  if (parts.some((part) => part === "Ctrl")) modifiers.push("Ctrl");
  if (parts.some((part) => part === "Shift")) modifiers.push("Shift");
  if (parts.some((part) => part === "Alt")) modifiers.push("Alt");
  if (parts.some((part) => part === "Meta")) modifiers.push("Meta");
  const key = parts.find((part) => !["Ctrl", "Shift", "Alt", "Meta"].includes(part)) ?? "";
  return [...modifiers, key].filter(Boolean).join("+");
}

export function eventShortcutFromKeyboardEvent(event) {
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

export function createInputSnapshot() {
  return {
    down: new Set(),
    events: [],
    sequence: 0,
  };
}

export function matchesInputShortcut(event, shortcut) {
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

export function createInputApi(moduleId, inputSnapshotRef, consumedInputRef) {
  const consumeCallCounts = new Map();

  function current() {
    return inputSnapshotRef.current ?? createInputSnapshot();
  }

  function consume(type, shortcut, scope = "") {
    const snapshot = current();
    const event = snapshot.events.find((item) => item.type === type && matchesInputShortcut(item, shortcut));
    if (!event) return false;
    const canonical = canonicalShortcut(shortcut);
    const callKey = `${type}:${canonical}:${event.id}`;
    const callIndex = consumeCallCounts.get(callKey) ?? 0;
    consumeCallCounts.set(callKey, callIndex + 1);
    const consumer = scope ? `scope:${String(scope)}` : `call:${callIndex}`;
    const key = `${moduleId}:${type}:${canonical}:${event.id}:${consumer}`;
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
    consumePress: (shortcut, scope) => consume("keydown", shortcut, scope),
    consumeRelease: (shortcut, scope) => consume("keyup", shortcut, scope),
    events: () => current().events.slice(),
    last: () => current().events[0] ?? null,
  };
}

export function collectModuleInputShortcuts(modules, config) {
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

export function normalizeShortcutBaseKey(event) {
  if (event.code?.startsWith("Numpad")) return event.code;
  if (event.key === " ") return "Space";
  if (event.key?.length === 1) return event.key.toUpperCase();
  return event.key || "";
}

export function normalizeKeyInput(event) {
  const baseKey = normalizeShortcutBaseKey(event);
  if (!baseKey || ["Control", "Shift", "Alt", "Meta", "OS"].includes(baseKey)) return "";

  const modifiers = [];
  if (event.ctrlKey) modifiers.push("Ctrl");
  if (event.shiftKey) modifiers.push("Shift");
  if (event.altKey) modifiers.push("Alt");
  if (event.metaKey) modifiers.push("Meta");

  return [...modifiers, baseKey].join("+");
}
