export function getWindowMode() {
  if (typeof window === "undefined") return "";
  const href = String(window.location.href ?? "");
  if (href.includes("window=game-overlay")) return "game-overlay";

  const searchMode = new URLSearchParams(window.location.search).get("window");
  if (searchMode) return searchMode;

  const hash = window.location.hash.startsWith("#")
    ? window.location.hash.slice(1)
    : window.location.hash;
  return new URLSearchParams(hash).get("window") ?? "";
}
