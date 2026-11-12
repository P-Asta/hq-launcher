import { spawn } from "node:child_process";
import { copyFile, mkdir } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { androidNativeLibrary } from "./android-native-library.mjs";

const root = fileURLToPath(new URL("../", import.meta.url));
const debug = process.argv.includes("--debug");
const profile = debug ? "debug" : "release";
const variant = debug ? "Debug" : "Release";

function run(command, args, cwd = root) {
  return new Promise((resolve, reject) => {
    let tail = "";
    const child = spawn(command, args, { cwd, stdio: ["inherit", "pipe", "pipe"] });
    for (const [stream, output] of [[child.stdout, process.stdout], [child.stderr, process.stderr]]) {
      stream.on("data", (data) => {
        output.write(data);
        tail = (tail + data).slice(-16000);
      });
    }
    child.on("error", reject);
    child.on("close", (code) => resolve({ code: code ?? 1, tail }));
  });
}

// Verify source/native provenance before Gradle consumes the embedded runtime.
const prepared = await run(process.env.PYTHON || "python", ["scripts/prepare-android-runtime.py"]);
if (prepared.code !== 0) process.exit(prepared.code);

// Let Tauri prepare the frontend, native library and generated Gradle config.
const result = await run(process.execPath, [
  "scripts/tauri-wrapper.mjs", "android", "build", "--apk", "--target", "aarch64", "--ci",
  ...(debug ? ["--debug"] : []),
]);

if (result.code === 0) process.exit(0);
if (process.platform !== "win32" || !result.tail.includes("Failed to create a symbolic link")) {
  process.exit(result.code);
}

// Windows without symlink privileges can package the exact same compiled .so.
// Skip only the already-completed Rust task; Gradle still runs all APK checks.
const android = path.join(root, "src-tauri/gen/android");
const nativeDir = path.join(android, "app/src/main/jniLibs/arm64-v8a");
await mkdir(nativeDir, { recursive: true });
await copyFile(
  await androidNativeLibrary(root, profile),
  path.join(nativeDir, "libhq_launcher_lib.so"),
);
console.log("Packaging Android APK using a copied native library (Windows symlink restriction).");
const packaged = await run("cmd.exe", [
  "/d", "/c", "gradlew.bat", `:app:assembleArm64${variant}`,
  "-x", `:app:rustBuildArm64${variant}`, "--console=plain",
], android);
process.exit(packaged.code);
