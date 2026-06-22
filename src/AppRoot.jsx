import { Component, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { AlertTriangle, Check, Copy, X } from "lucide-react";
import GameOverlay from "./GameOverlay";
import LauncherPage from "./pages/LauncherPage";
import { LoginDialog } from "./components/auth/LoginDialog";
import { UpdateDialog } from "./components/UpdateDialog";
import Titlebar from "./Titlebar";
import { Dialog, DialogContent } from "./components/ui/dialog";
import { getWindowMode } from "./lib/windowMode";

function Splash({ message }) {
  return (
    <div className="flex h-full w-full items-center justify-center p-6 text-white">
      <div className="w-[min(520px,92vw)] rounded-3xl border border-panel-outline bg-white/5 p-6">
        <div className="text-lg font-semibold">HQ Launcher</div>
        <div className="mt-2 text-sm text-white/55">{message}</div>
      </div>
    </div>
  );
}

function formatErrorTime(timestampMs) {
  const numeric = Number(timestampMs);
  if (!Number.isFinite(numeric) || numeric <= 0) return "";
  return new Date(numeric).toLocaleString();
}

function errorText(error) {
  const location =
    error?.file && error?.line ? `${error.file}:${error.line}` : error?.file || "";
  return [
    `[${formatErrorTime(error?.timestamp_ms)}] ${error?.message ?? ""}`,
    error?.target ? `target: ${error.target}` : "",
    error?.module_path ? `module: ${error.module_path}` : "",
    location ? `location: ${location}` : "",
  ]
    .filter(Boolean)
    .join("\n");
}

function ErrorInbox({ errors }) {
  const [open, setOpen] = useState(false);
  const [copied, setCopied] = useState("");

  if (errors.length === 0) return null;

  async function copyText(id, text) {
    await navigator.clipboard.writeText(text);
    setCopied(id);
    window.setTimeout(() => {
      setCopied((current) => (current === id ? "" : current));
    }, 1200);
  }

  const allText = errors.map(errorText).join("\n\n---\n\n");

  return (
    <>
      <button
        type="button"
        onClick={() => setOpen(true)}
        className="fixed bottom-4 right-4 z-50 flex max-w-[calc(100vw-2rem)] items-center gap-2 rounded-xl border border-red-400/35 bg-red-500/15 px-3 py-2 text-left text-sm text-red-100 shadow-xl shadow-black/35 transition hover:border-red-300/60 hover:bg-red-500/25 focus:outline-none focus:ring-2 focus:ring-red-300/45"
      >
        <AlertTriangle className="h-4 w-4 shrink-0" />
        <span className="font-semibold">Errors</span>
        <span className="rounded-md bg-red-400/20 px-1.5 py-0.5 text-xs font-semibold">
          {errors.length}
        </span>
      </button>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="max-h-[calc(100vh-5rem)] w-[min(780px,94vw)] overflow-hidden p-0">
          <div className="flex items-center justify-between border-b border-panel-outline px-5 py-4">
            <div>
              <div className="text-base font-semibold">Application Errors</div>
              <div className="mt-1 text-xs text-white/45">
                {errors.length} error{errors.length === 1 ? "" : "s"} captured this session
              </div>
            </div>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={() => copyText("all", allText).catch(console.error)}
                className="inline-flex h-8 items-center gap-2 rounded-lg border border-panel-outline bg-white/5 px-3 text-xs font-medium text-white/75 transition hover:bg-white/10 hover:text-white"
              >
                {copied === "all" ? <Check className="h-3.5 w-3.5" /> : <Copy className="h-3.5 w-3.5" />}
                Copy all
              </button>
              <button
                type="button"
                onClick={() => setOpen(false)}
                className="inline-flex h-8 w-8 items-center justify-center rounded-lg text-white/55 transition hover:bg-white/10 hover:text-white"
                aria-label="Close errors"
              >
                <X className="h-4 w-4" />
              </button>
            </div>
          </div>
          <div className="max-h-[calc(100vh-12rem)] overflow-auto p-4">
            <div className="flex flex-col gap-3">
              {errors.map((error, index) => {
                const id = `${error.timestamp_ms ?? index}-${index}`;
                const text = errorText(error);
                return (
                  <div
                    key={id}
                    className="rounded-xl border border-red-300/20 bg-black/20 p-3"
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div className="min-w-0">
                        <div className="text-sm font-semibold text-red-100">
                          {error.message || "Unknown error"}
                        </div>
                        <div className="mt-1 text-xs text-white/40">
                          {formatErrorTime(error.timestamp_ms)}
                        </div>
                      </div>
                      <button
                        type="button"
                        onClick={() => copyText(id, text).catch(console.error)}
                        className="inline-flex h-8 shrink-0 items-center gap-2 rounded-lg border border-panel-outline bg-white/5 px-2.5 text-xs text-white/70 transition hover:bg-white/10 hover:text-white"
                      >
                        {copied === id ? <Check className="h-3.5 w-3.5" /> : <Copy className="h-3.5 w-3.5" />}
                        Copy
                      </button>
                    </div>
                    <pre className="mt-3 whitespace-pre-wrap break-words rounded-lg bg-black/25 p-3 text-xs leading-relaxed text-white/65">
                      {text}
                    </pre>
                  </div>
                );
              })}
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}

function LauncherRoot() {
  const [loginState, setLoginState] = useState({
    status: "loading", // loading | ready
    is_logged_in: false,
    username: null,
    steam_id: null,
  });
  const [bootstrapError, setBootstrapError] = useState("");
  const [loginOpen, setLoginOpen] = useState(false);
  const loginResolveRef = useRef(null);
  const [updateInfo, setUpdateInfo] = useState(null);
  const [updateDialogOpen, setUpdateDialogOpen] = useState(false);
  const updateCheckedRef = useRef(false);
  const [installedVersions, setInstalledVersions] = useState([]);
  const [appErrors, setAppErrors] = useState([]);

  async function checkForAppUpdate() {
    const info = await invoke("check_app_update");
    if (info?.available) {
      setUpdateInfo(info);
      setUpdateDialogOpen(true);
    } else {
      setUpdateInfo(info ?? null);
      setUpdateDialogOpen(false);
    }
    return info;
  }

  async function refreshLoginState() {
    try {
      const s = await invoke("depot_get_login_state");
      setLoginState({
        status: "ready",
        is_logged_in: !!s?.is_logged_in,
        username: s?.username ?? null,
        steam_id: s?.steam_id ?? null,
      });
      setBootstrapError("");
    } catch (e) {
      setLoginState({
        status: "ready",
        is_logged_in: false,
        username: null,
        steam_id: null,
      });
      setBootstrapError(e?.message ?? String(e));
    }
  }

  useEffect(() => {
    refreshLoginState();
  }, []);

  useEffect(() => {
    let unlisten = null;
    let disposed = false;

    (async () => {
      unlisten = await listen("app-error://created", (event) => {
        if (disposed) return;
        setAppErrors((current) => [...current, event.payload]);
      });
    })().catch(console.error);

    return () => {
      disposed = true;
      if (typeof unlisten === "function") unlisten();
    };
  }, []);

  // Check for updates on app startup
  useEffect(() => {
    if (updateCheckedRef.current) return;
    updateCheckedRef.current = true;

    (async () => {
      try {
        await checkForAppUpdate();
      } catch (e) {
        console.error("Failed to check for updates:", e);
        // Silently ignore update check failures to avoid disrupting user experience
      }
    })();
  }, []);

  useEffect(() => {
    let unlisten = null;
    let disposed = false;

    (async () => {
      unlisten = await listen("release-channel://changed", async () => {
        try {
          const info = await checkForAppUpdate();
          if (!disposed && !info?.available) {
            setUpdateDialogOpen(false);
          }
        } catch (e) {
          console.error("Failed to check for channel update:", e);
        }
      });
    })();

    return () => {
      disposed = true;
      if (typeof unlisten === "function") unlisten();
    };
  }, []);

  function requestLogin() {
    setLoginOpen(true);
    return new Promise((resolve) => {
      loginResolveRef.current = resolve;
    });
  }

  function closeLoginDialog(result = false) {
    setLoginOpen(false);
    if (loginResolveRef.current) loginResolveRef.current(result);
    loginResolveRef.current = null;
  }

  async function logout() {
    try {
      await invoke("depot_logout");
    } catch {}
    refreshLoginState();
  }

  return (
    <div className="h-full w-full overflow-hidden bg-[var(--theme-bg)]">
      <Titlebar className="fixed top-0 left-0 h-10" installedVersions={installedVersions} />
      <div className="relative mt-10 h-[calc(100vh-40px)] w-full">
        {loginState.status === "loading" ? (
          <Splash message="Starting up..." />
        ) : (
          <LauncherPage
            loginState={loginState}
            onLogout={logout}
            onRequireLogin={requestLogin}
            bootstrapError={bootstrapError}
            onInstalledVersionsChange={setInstalledVersions}
          />
        )}

        <LoginDialog
          open={loginOpen}
          onOpenChange={(nextOpen) => {
            if (!nextOpen) closeLoginDialog(false);
          }}
          onLoggedIn={(s) => {
            setLoginState({
              status: "ready",
              is_logged_in: !!s?.is_logged_in,
              username: s?.username ?? null,
              steam_id: s?.steam_id ?? null,
            });
            closeLoginDialog(true);
          }}
        />

        <UpdateDialog
          open={updateDialogOpen}
          onOpenChange={setUpdateDialogOpen}
          updateInfo={updateInfo}
        />
        <ErrorInbox errors={appErrors} />
      </div>
    </div>
  );
}

function OverlayEmergencyFallback({ error, onRetry }) {
  const [controlsOpen, setControlsOpen] = useState(false);
  const [copied, setCopied] = useState(false);
  const stack = String(error?.stack ?? error?.message ?? error);
  const shortStack = stack.split("\n").slice(0, 8).join("\n");

  useEffect(() => {
    let unlisten = null;
    let disposed = false;
    listen("overlay://controls-open-changed", (event) => {
      if (!disposed) setControlsOpen(!!event.payload);
    }).then((nextUnlisten) => {
      if (disposed) {
        nextUnlisten();
      } else {
        unlisten = nextUnlisten;
      }
    }).catch(console.error);
    return () => {
      disposed = true;
      if (typeof unlisten === "function") unlisten();
    };
  }, []);

  function closeControls() {
    invoke("set_game_overlay_controls_open", { open: false }).catch(console.error);
  }

  async function copyError() {
    await navigator.clipboard.writeText(stack);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1200);
  }

  if (!controlsOpen) {
    return (
      <div className="pointer-events-none fixed right-4 top-4 z-[2147483647] max-w-[min(360px,calc(100vw-2rem))] text-white">
        <div className="rounded border border-red-300/35 bg-red-950/75 px-3 py-2 text-xs shadow-2xl shadow-black/60">
          <div className="font-semibold text-red-100">Overlay crashed</div>
          <div className="mt-1 text-red-100/70">Focus Lethal Company and press the overlay key to open recovery.</div>
        </div>
      </div>
    );
  }

  return (
    <div className="fixed inset-0 z-[2147483647] bg-black/35 text-white">
      <div className="absolute right-6 top-6 flex w-[min(620px,calc(100vw-3rem))] flex-col overflow-hidden rounded border border-red-300/35 bg-[#121318]/98 shadow-2xl shadow-black/70">
        <div className="flex items-center justify-between border-b border-white/10 px-4 py-3">
          <div>
            <div className="text-sm font-semibold text-red-100">Overlay Recovery</div>
            <div className="text-xs text-white/45">Game input is protected; this panel only appears when the overlay menu is open.</div>
          </div>
          <button
            type="button"
            onClick={closeControls}
            className="flex h-8 w-8 items-center justify-center rounded text-white/55 hover:bg-white/10 hover:text-white"
            aria-label="Close overlay recovery"
          >
            <X className="h-4 w-4" />
          </button>
        </div>
        <div className="p-4">
          <div className="mb-3 rounded border border-red-300/20 bg-red-950/35 px-3 py-2 text-sm text-red-100">
            GameOverlay render failed: {String(error?.message ?? error)}
          </div>
          <pre className="max-h-64 overflow-auto whitespace-pre-wrap break-words rounded bg-black/35 p-3 font-mono text-[11px] leading-relaxed text-white/65">
            {shortStack}
          </pre>
          <div className="mt-4 flex flex-wrap justify-end gap-2">
            <button
              type="button"
              onClick={() => copyError().catch(console.error)}
              className="rounded border border-white/10 bg-white/5 px-3 py-2 text-xs text-white/75 hover:bg-white/10 hover:text-white"
            >
              {copied ? "Copied" : "Copy error"}
            </button>
            <button
              type="button"
              onClick={onRetry}
              className="rounded border border-white/10 bg-white/5 px-3 py-2 text-xs text-white/75 hover:bg-white/10 hover:text-white"
            >
              Retry overlay
            </button>
            <button
              type="button"
              onClick={() => window.location.reload()}
              className="rounded bg-red-300 px-3 py-2 text-xs font-semibold text-black hover:bg-red-200"
            >
              Reload overlay
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

class OverlayErrorBoundary extends Component {
  constructor(props) {
    super(props);
    this.state = { error: null };
  }

  static getDerivedStateFromError(error) {
    return { error };
  }

  componentDidCatch(error, info) {
    invoke("set_game_overlay_controls_open", { open: false }).catch(console.error);
    invoke("report_game_overlay_frontend_error", {
      message: `GameOverlay render failed: ${error?.message ?? error}\n${info?.componentStack ?? ""}`,
    }).catch(console.error);
  }

  render() {
    if (this.state.error) {
      return (
        <OverlayEmergencyFallback
          error={this.state.error}
          onRetry={() => this.setState({ error: null })}
        />
      );
    }

    return this.props.children;
  }
}

export default function AppRoot() {
  const windowMode = getWindowMode();
  const isGameOverlay = windowMode === "game-overlay";
  const isObsOverlay = windowMode === "obs-overlay";

  if (isGameOverlay || isObsOverlay) {
    invoke("report_game_overlay_frontend_info", { message: `AppRoot rendering ${windowMode}` }).catch(console.error);
    return (
      <OverlayErrorBoundary>
        <GameOverlay captureOnly={isObsOverlay} />
      </OverlayErrorBoundary>
    );
  }

  return <LauncherRoot />;
}
