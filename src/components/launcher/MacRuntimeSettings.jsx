import { useCallback, useEffect, useId, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { CheckCircle2, Download, FolderOpen, LoaderCircle, RefreshCw } from "lucide-react";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "../ui/select";

const errorMessage = (error) => error?.message || String(error);
const isComplete = (progress) => /^(ready|done|complete|completed)$/i.test(progress?.stage || "");
const isFailed = (progress) => /^(error|failed)$/i.test(progress?.stage || "");

export function MacRuntimeProgress({ onOpenSettings }) {
  const [progress, setProgress] = useState(null);
  useEffect(() => {
    let cancelled = false;
    let unlisten;
    listen("macos-runtime-progress", ({ payload }) => {
      if (!cancelled) setProgress(isComplete(payload) ? null : payload);
    }).then((stop) => {
      if (cancelled) stop();
      else unlisten = stop;
    }).catch(() => {});
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  if (!progress) return null;
  const failed = isFailed(progress);
  const percent = Number.isFinite(progress.percent) ? Math.max(0, Math.min(100, progress.percent)) : null;
  return (
    <div className={`flex items-center gap-3 rounded-xl border px-3 py-2 text-sm ${failed ? "border-red-400/30 bg-red-500/10 text-red-100" : "border-panel-outline bg-white/[0.04] text-white/75"}`} role={failed ? "alert" : "status"} aria-live="polite">
      {!failed ? <LoaderCircle className="h-4 w-4 shrink-0 animate-spin" /> : null}
      <div className="min-w-0 flex-1">
        <div>{failed ? "macOS runtime setup failed" : "Preparing macOS game support"}{percent != null && !failed ? ` · ${Math.round(percent)}%` : ""}</div>
        <div className="mt-0.5 break-words text-xs opacity-70">{progress.message || (failed ? "Open Launch settings to retry setup." : progress.stage)}</div>
      </div>
      <Button type="button" variant="outline" size="sm" onClick={onOpenSettings} className="shrink-0">Launch settings</Button>
    </div>
  );
}

export function MacRuntimeSettings() {
  const id = useId();
  const [status, setStatus] = useState(null);
  const [runtimePath, setRuntimePath] = useState("");
  const [graphicsBackend, setGraphicsBackend] = useState("auto");
  const [prefixPath, setPrefixPath] = useState("");
  const [action, setAction] = useState(null);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [progress, setProgress] = useState(null);

  const applyStatus = useCallback((next, updatePaths = true) => {
    setStatus(next);
    if (updatePaths) {
      setRuntimePath(next?.runtimePath || "");
      setGraphicsBackend(next?.graphicsBackend || "auto");
      setPrefixPath(next?.prefixPath || "");
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    let unlisten;
    const refresh = () => invoke("get_macos_runtime_status").then((next) => {
      if (!cancelled) applyStatus(next);
    });

    refresh().catch((err) => {
      if (!cancelled) setError(errorMessage(err));
    });
    listen("macos-runtime-progress", ({ payload }) => {
      if (cancelled) return;
      setProgress(isComplete(payload) || isFailed(payload) ? null : payload);
      if (isFailed(payload)) setError(payload.message || "macOS runtime setup failed. Try installing the runtime again.");
      if (isComplete(payload)) {
        invoke("get_macos_runtime_status").then((next) => {
          if (!cancelled) applyStatus(next, false);
        }).catch((err) => {
          if (!cancelled) setError(errorMessage(err));
        });
      }
    }).then((stop) => {
      if (cancelled) stop();
      else unlisten = stop;
    }).catch((err) => {
      if (!cancelled) setError(errorMessage(err));
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [applyStatus]);

  async function perform(name, operation) {
    setAction(name);
    setError("");
    setMessage("");
    try {
      await operation();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setAction(null);
    }
  }

  function refresh() {
    return perform("refresh", async () => {
      applyStatus(await invoke("get_macos_runtime_status"));
      setProgress(null);
    });
  }

  function install() {
    return perform("install", async () => {
      setProgress({ stage: "Preparing macOS runtime", percent: null });
      try {
        applyStatus(await invoke("install_macos_runtime"));
        setMessage("The macOS runtime is installed. Open Windows Steam to finish setup and sign in.");
      } finally {
        setProgress(null);
      }
    });
  }

  function openSteam() {
    return perform("steam", async () => {
      await invoke("start_macos_steam");
      applyStatus(await invoke("get_macos_runtime_status"), false);
      setMessage("Windows Steam is installed and has opened. Wait for its updates and sign in, then press Run.");
    });
  }

  function save(automatic = false) {
    return perform("save", async () => {
      applyStatus(await invoke("set_macos_runtime_settings", {
        runtimePath: automatic ? "" : runtimePath.trim(),
        prefixPath: automatic ? "" : prefixPath.trim(),
        graphicsBackend: automatic ? "auto" : graphicsBackend,
      }));
      setMessage(automatic ? "Automatic setup selected. The launcher will prepare its own runtime and environment when needed." : "macOS runtime settings saved.");
    });
  }

  function browse(kind) {
    return perform(kind, async () => {
      const path = await invoke("choose_macos_runtime_path", { kind });
      if (path) {
        if (kind === "runtime") setRuntimePath(path);
        else setPrefixPath(path);
      }
    });
  }

  if (!status) {
    if (!error) return null;
    return (
      <section className="space-y-3 rounded-2xl border border-panel-outline bg-black/20 p-4">
        <h3 className="text-sm font-medium text-white">macOS game support</h3>
        <div role="alert" className="break-words text-xs text-red-200">{error}</div>
        <Button type="button" size="sm" disabled={action != null} onClick={refresh}>Retry</Button>
      </section>
    );
  }

  const busy = action != null;
  const runtimes = status.detectedRuntimes || [];
  const prefixes = status.detectedPrefixes || [];
  const runtimeInstalled = status.runtimeInstalled ?? !!status.runtimePath;
  const percent = Number.isFinite(progress?.percent)
    ? Math.max(0, Math.min(100, progress.percent))
    : null;

  return (
    <section className="space-y-4 rounded-2xl border border-panel-outline bg-black/20 p-4" aria-labelledby={`${id}-title`}>
      <div className="flex items-start justify-between gap-3">
        <div>
          <h3 id={`${id}-title`} className="text-sm font-medium text-white">macOS game support</h3>
          <p className="mt-1 text-xs leading-relaxed text-white/55">
            The launcher downloads and sets up Wine automatically so you can run the Windows game on macOS.
          </p>
        </div>
        <Button type="button" variant="ghost" size="sm" disabled={busy} onClick={refresh} aria-label="Refresh macOS runtime status" title="Refresh status" className="shrink-0 px-2">
          <RefreshCw className={`h-4 w-4 ${action === "refresh" ? "animate-spin" : ""}`} />
        </Button>
      </div>

      <div className="flex flex-wrap gap-x-4 gap-y-2 text-xs">
        <span className={runtimeInstalled ? "text-[var(--theme-accent)]" : "text-white/55"}>
          {runtimeInstalled ? "Runtime installed" : "Runtime will install automatically"}
        </span>
        <span className={status.steamInstalled ? "text-[var(--theme-accent)]" : "text-white/55"}>
          {status.steamInstalled ? "Windows Steam installed" : "Windows Steam setup required"}
        </span>
      </div>

      {progress ? (
        <div className="space-y-2 text-xs text-white/65" role="status" aria-live="polite">
          <div className="flex items-center gap-2">
            <LoaderCircle className="h-3.5 w-3.5 shrink-0 animate-spin" />
            <span className="min-w-0 flex-1">{progress.message || progress.stage || "Preparing macOS runtime"}</span>
            {percent != null ? <span>{Math.round(percent)}%</span> : null}
          </div>
          {percent != null ? (
            <div className="h-1.5 overflow-hidden rounded-full bg-white/10" role="progressbar" aria-label="macOS runtime installation" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(percent)}>
              <div className="h-full rounded-full bg-[var(--theme-accent)] transition-[width]" style={{ width: `${percent}%` }} />
            </div>
          ) : null}
        </div>
      ) : null}

      <div className="flex flex-wrap gap-2">
        {!runtimeInstalled ? (
          <Button type="button" size="sm" variant="secondary" disabled={busy} onClick={install}>
            {action === "install" ? <LoaderCircle className="h-4 w-4 animate-spin" /> : <Download className="h-4 w-4" />}
            Install runtime
          </Button>
        ) : null}
        <Button type="button" size="sm" variant="default" disabled={busy} onClick={openSteam}>
          {action === "steam" ? <LoaderCircle className="h-4 w-4 animate-spin" /> : status.steamInstalled ? <CheckCircle2 className="h-4 w-4" /> : <Download className="h-4 w-4" />}
          {status.steamInstalled ? "Open Windows Steam" : "Install Windows Steam"}
        </Button>
      </div>

      <p className="text-xs leading-relaxed text-white/55">
        Sign in to Windows Steam in this Wine environment before running the game. Steam for macOS cannot replace Windows Steam. If you use CrossOver, Steam and the game must use the same bottle.
      </p>

      {status.metalSupported ? (
        <div className="space-y-2 border-t border-panel-outline pt-3">
          <label className="text-xs font-medium text-white/75" htmlFor={`${id}-graphics`}>Game graphics</label>
          <Select value={graphicsBackend} onValueChange={setGraphicsBackend} disabled={busy}>
            <SelectTrigger id={`${id}-graphics`}><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="auto">Metal — recommended</SelectItem>
              <SelectItem value="dxvk">DXVK — compatibility</SelectItem>
            </SelectContent>
          </Select>
          <p className="text-xs leading-relaxed text-white/55">Metal is installed automatically for the managed runtime. Shader caches are reused between launches. Changes apply the next time you run the game.</p>
          <Button type="button" size="sm" variant="outline" disabled={busy} onClick={() => save(false)}>Save graphics setting</Button>
        </div>
      ) : null}

      <details className="border-t border-panel-outline pt-3">
        <summary className="cursor-pointer text-xs font-medium text-white/65 hover:text-white">Advanced: custom Wine / CrossOver</summary>
        <div className="mt-4 space-y-4">
          <p className="text-xs leading-relaxed text-white/45">
            Automatic setup works without custom paths. Current paths are shown below; leave either path empty to use its managed default.
          </p>
          <div className="space-y-2">
            <label htmlFor={`${id}-runtime`} className="text-xs font-medium text-white/75">Wine runtime or CrossOver app</label>
            {runtimes.length ? (
              <Select value={runtimes.some((item) => item.path === runtimePath) ? runtimePath : ""} onValueChange={setRuntimePath} disabled={busy}>
                <SelectTrigger aria-label="Detected Wine runtimes"><SelectValue placeholder="Choose a detected runtime" /></SelectTrigger>
                <SelectContent>{runtimes.map((item) => <SelectItem key={item.path} value={item.path}>{item.label}</SelectItem>)}</SelectContent>
              </Select>
            ) : null}
            <div className="flex gap-2">
              <Input id={`${id}-runtime`} value={runtimePath} onChange={(event) => setRuntimePath(event.target.value)} placeholder="Automatic (managed Wine)" disabled={busy} className="min-w-0 font-mono text-xs" />
              <Button type="button" variant="secondary" disabled={busy} onClick={() => browse("runtime")} className="shrink-0 px-3" aria-label="Browse for Wine runtime" title="Browse for Wine runtime"><FolderOpen className="h-4 w-4" /></Button>
            </div>
          </div>
          <div className="space-y-2">
            <label htmlFor={`${id}-prefix`} className="text-xs font-medium text-white/75">Wine prefix / CrossOver bottle</label>
            {prefixes.length ? (
              <Select value={prefixes.some((item) => item.path === prefixPath) ? prefixPath : ""} onValueChange={setPrefixPath} disabled={busy}>
                <SelectTrigger aria-label="Detected Wine environments"><SelectValue placeholder="Choose a detected environment" /></SelectTrigger>
                <SelectContent>{prefixes.map((item) => <SelectItem key={item.path} value={item.path}>{item.label}</SelectItem>)}</SelectContent>
              </Select>
            ) : null}
            <div className="flex gap-2">
              <Input id={`${id}-prefix`} value={prefixPath} onChange={(event) => setPrefixPath(event.target.value)} placeholder="Automatic (launcher environment)" disabled={busy} className="min-w-0 font-mono text-xs" />
              <Button type="button" variant="secondary" disabled={busy} onClick={() => browse("prefix")} className="shrink-0 px-3" aria-label="Browse for Wine prefix or CrossOver bottle" title="Browse for Wine prefix or CrossOver bottle"><FolderOpen className="h-4 w-4" /></Button>
            </div>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button type="button" size="sm" disabled={busy} onClick={() => save(false)}>{action === "save" ? <LoaderCircle className="h-4 w-4 animate-spin" /> : null}Save custom settings</Button>
            <Button type="button" size="sm" variant="ghost" disabled={busy} onClick={() => save(true)}>Use automatic setup</Button>
          </div>
        </div>
      </details>

      {error ? <div role="alert" className="break-words rounded-xl border border-red-400/30 bg-red-400/10 px-3 py-2 text-xs text-red-200">{error}</div> : null}
      {message ? <div role="status" className="rounded-xl border border-panel-outline bg-white/[0.04] px-3 py-2 text-xs leading-relaxed text-white/75">{message}</div> : null}
    </section>
  );
}
