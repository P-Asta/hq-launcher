import path from "node:path";
import { execFile } from "node:child_process";
import { promisify } from "node:util";

const execute = promisify(execFile);

export async function androidNativeLibrary(root, profile) {
  const { stdout } = await execute("cargo", ["metadata", "--no-deps", "--format-version", "1", "--offline"], {
    cwd: path.join(root, "src-tauri"),
  });
  const metadata = JSON.parse(stdout);
  return path.join(metadata.target_directory, "aarch64-linux-android", profile, "libhq_launcher_lib.so");
}
