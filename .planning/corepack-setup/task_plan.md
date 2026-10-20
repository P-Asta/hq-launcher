# Yarn/Corepack setup

1. Diagnose command paths and permissions (done).
2. Enable user-level Yarn shims and cache Yarn 3.6.4 (done).
3. Verify install, Tauri command, and build (done).

Verified: yarn --version = 3.6.4; yarn install --immutable exit 0; yarn tauri dev --help exit 0; yarn build exit 0; nested yarn invocation = 3.6.4. Project packageManager and lockfiles unchanged. Full desktop dev app was not launched.
