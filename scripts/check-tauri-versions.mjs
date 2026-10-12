import { readFileSync } from "node:fs";

const root = new URL("../", import.meta.url);
const lock = readFileSync(new URL("src-tauri/Cargo.lock", root), "utf8");
const crates = new Map(lock.split("[[package]]").flatMap((entry) => {
  const name = entry.match(/^name = "([^"]+)"/m)?.[1];
  const version = entry.match(/^version = "([^"]+)"/m)?.[1];
  return name && version ? [[name, version]] : [];
}));
const pairs = [
  ["api", "tauri"],
  ["plugin-global-shortcut", "tauri-plugin-global-shortcut"],
  ["plugin-opener", "tauri-plugin-opener"],
  ["plugin-updater", "tauri-plugin-updater"],
];
for (const [npmName, crateName] of pairs) {
  const { version } = JSON.parse(readFileSync(
    new URL(`node_modules/@tauri-apps/${npmName}/package.json`, root), "utf8",
  ));
  const rustVersion = crates.get(crateName);
  if (!rustVersion || version.split(".").slice(0, 2).join(".") !== rustVersion.split(".").slice(0, 2).join(".")) {
    throw new Error(`Tauri version mismatch: @tauri-apps/${npmName} ${version}, ${crateName} ${rustVersion ?? "missing"}`);
  }
  console.log(`OK: @tauri-apps/${npmName} ${version} / ${crateName} ${rustVersion}`);
}
