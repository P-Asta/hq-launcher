import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { androidNativeLibrary } from "./android-native-library.mjs";

// An old default-target library must never replace the current library when
// Cargo builds to a configured target directory. Use real Cargo metadata.
const fixture = await mkdtemp(path.join(tmpdir(), "hq-android-build-"));
const previousTarget = process.env.CARGO_TARGET_DIR;
delete process.env.CARGO_TARGET_DIR;
try {
  const rust = path.join(fixture, "src-tauri");
  await mkdir(path.join(rust, "src"), { recursive: true });
  await mkdir(path.join(rust, ".cargo"));
  await writeFile(path.join(rust, "Cargo.toml"), '[package]\nname="hq-android-build-fixture"\nversion="0.1.0"\nedition="2021"\n');
  await writeFile(path.join(rust, "src/lib.rs"), "pub fn fixture() {}\n");
  await writeFile(path.join(rust, ".cargo/config.toml"), '[build]\ntarget-dir="../android-output"\n');
  for (const [directory, content] of [[path.join(rust, "target"), "stale"], [path.join(fixture, "android-output"), "current"]]) {
    const native = path.join(directory, "aarch64-linux-android/debug");
    await mkdir(native, { recursive: true });
    await writeFile(path.join(native, "libhq_launcher_lib.so"), content);
  }
  assert.equal(await readFile(await androidNativeLibrary(fixture, "debug"), "utf8"), "current");

  process.env.CARGO_TARGET_DIR = path.join(fixture, "environment-target");
  assert.equal(await androidNativeLibrary(fixture, "release"), path.join(fixture, "environment-target/aarch64-linux-android/release/libhq_launcher_lib.so"));
  console.log("Android packaging uses Cargo's configured target directory; stale default library rejected.");
} finally {
  if (previousTarget === undefined) delete process.env.CARGO_TARGET_DIR;
  else process.env.CARGO_TARGET_DIR = previousTarget;
  const resolved = path.resolve(fixture);
  if (!resolved.startsWith(path.resolve(tmpdir()) + path.sep) || !path.basename(resolved).startsWith("hq-android-build-")) throw new Error("Unexpected fixture path");
  await rm(resolved, { recursive: true, force: true });
}
