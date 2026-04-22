import { spawn } from "node:child_process";
import process from "node:process";
import { createRequire } from "node:module";

const args = process.argv.slice(2);
const env = { ...process.env };
const require = createRequire(import.meta.url);
const tauriCliEntry = require.resolve("@tauri-apps/cli/tauri.js");

if (process.platform === "linux" && env.NO_STRIP == null) {
  env.NO_STRIP = "true";
}

const child = spawn(process.execPath, [tauriCliEntry, ...args], {
  stdio: "inherit",
  env,
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }

  process.exit(code ?? 0);
});
