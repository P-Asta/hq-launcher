import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Download, Play, RefreshCw, Settings, Square } from "lucide-react";
import { Button } from "../components/ui/button";
import { Switch } from "../components/ui/switch";
import { Select, SelectTrigger, SelectContent, SelectItem } from "../components/ui/select";
import { ModCover } from "../components/launcher/ModCover";
import { HqLoading, HqLogo } from "../components/launcher/HqLoading";
import { LauncherWorkspace, LauncherToolbar, LauncherSearch, LauncherModRow, useCompactLauncher } from "../components/launcher/LauncherWorkspace";

const errorMessage = (cause) => cause?.message ?? String(cause);
const progressValue = (value) => typeof value === "number" && value >= 0 ? Math.min(100, value) : undefined;

function InstallProgress({ label, progress, detail }) {
  const value = progressValue(progress);
  return (
    <div className="mt-3 space-y-2">
      <HqLoading title={label} detail={detail || "Preparing…"} progress={value} />
      <div role="progressbar" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={value} aria-valuetext={value === undefined ? "In progress" : `${value}%`} className="h-1.5 overflow-hidden rounded-full bg-white/10">
        <div className="h-full rounded-full bg-[var(--theme-accent)] transition-[width]" style={{ width: value === undefined ? "25%" : `${value}%` }} />
      </div>
      {value !== undefined && <p className="text-xs text-white/60">{Math.round(value)}%</p>}
    </div>
  );
}

export default function AndroidLauncher() {
  const compactLayout = useCompactLauncher();
  const [query, setQuery] = useState("");
  const [runtime, setRuntime] = useState(null);
  const [checking, setChecking] = useState(true);
  const [mods, setMods] = useState(null);
  const [modsChecking, setModsChecking] = useState(false);
  const [versions, setVersions] = useState([]);
  const [versionError, setVersionError] = useState("");
  const [busy, setBusy] = useState("");
  const [statusError, setStatusError] = useState("");
  const [modError, setModError] = useState("");
  const [actionError, setActionError] = useState("");
  const [notice, setNotice] = useState("");
  const [modProgress, setModProgress] = useState(null);
  const [watchUntil, setWatchUntil] = useState(0);
  const active = useRef(true);
  const versionRequest = useRef(0);
  const versionSelection = useRef(null);
  const runtimeRef = useRef(null);
  const request = useRef(0);
  const modRequest = useRef(0);
  const actionLock = useRef("");
  const progressListener = useRef(null);

  const refreshMods = useCallback(async () => {
    const id = ++modRequest.current;
    setModsChecking(true);
    try {
      const next = await invoke("get_android_mod_state");
      if (active.current && id === modRequest.current && runtimeRef.current?.game_installed) {
        setMods(next);
        setModError("");
      }
    } catch (cause) {
      if (active.current && id === modRequest.current) {
        setMods(null);
        setModError(errorMessage(cause));
      }
    } finally {
      if (active.current && id === modRequest.current) setModsChecking(false);
    }
  }, []);

  const refreshVersions = useCallback(async () => {
    const id = ++versionRequest.current;
    try {
      const next = await invoke("get_android_versions");
      if (active.current && id === versionRequest.current) {
        const selected = next.versions?.find(item => item.selected)?.version ?? null;
        const changed = versionSelection.current !== selected;
        versionSelection.current = selected;
        setVersions(next.versions || []);
        setVersionError("");
        return changed;
      }
    } catch (cause) {
      if (active.current && id === versionRequest.current) {
        setVersions([]);
        versionSelection.current = null;
        setVersionError(errorMessage(cause));
      }
    }
    return false;
  }, []);

  const refresh = useCallback(async ({ quiet = false, reloadMods = true, afterAction = false } = {}) => {
    const id = ++request.current;
    if (!quiet) setChecking(true);
    try {
      const next = await invoke("get_android_runtime_status");
      if (!active.current || id !== request.current) return;
      const previous = runtimeRef.current;
      runtimeRef.current = next;
      setRuntime(next);
      setStatusError("");
      let selectionChanged = false;
      if (next?.runtime_installed && next?.steam_installed) selectionChanged = await refreshVersions();
      else { ++versionRequest.current; versionSelection.current = null; setVersions([]); setVersionError(""); }
      if (!active.current || id !== request.current) return;
      if (!next?.game_installed) {
        ++modRequest.current;
        setMods(null);
        setModError("");
        setModsChecking(false);
      } else if ((reloadMods || selectionChanged || (previous?.version_installing && !next.version_installing) || !previous?.game_installed) && (afterAction || !["install_android_mods", "set_android_mod_enabled"].includes(actionLock.current))) {
        await refreshMods();
      }
    } catch (cause) {
      if (active.current && id === request.current) {
        runtimeRef.current = null;
        setRuntime(null);
        ++versionRequest.current;
        setVersions([]);
        versionSelection.current = null;
        ++modRequest.current;
        setMods(null);
        setModsChecking(false);
        setNotice("");
        setStatusError(errorMessage(cause));
      }
    } finally {
      if (active.current && id === request.current) setChecking(false);
    }
  }, [refreshMods, refreshVersions]);

  useEffect(() => {
    active.current = true;
    document.documentElement.lang = "en";
    void refresh();
    const onVisible = () => { if (!document.hidden) void refresh(); };
    window.addEventListener("focus", onVisible);
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      active.current = false;
      ++request.current;
      ++modRequest.current;
      ++versionRequest.current;
      progressListener.current?.().catch(() => {});
      progressListener.current = null;
      window.removeEventListener("focus", onVisible);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [refresh]);

  useEffect(() => {
    if (!runtime?.installing && !runtime?.version_installing && !runtime?.session_running && !mods?.applying && !busy && watchUntil <= Date.now()) return;
    let cancelled = false;
    let timer;
    const poll = async () => {
      if (!document.hidden) await refresh({ quiet: true, reloadMods: Boolean(mods?.applying) });
      if (!cancelled && (runtimeRef.current?.installing || runtimeRef.current?.version_installing || runtimeRef.current?.session_running || mods?.applying || actionLock.current || watchUntil > Date.now())) timer = window.setTimeout(poll, 2000);
    };
    timer = window.setTimeout(poll, 2000);
    return () => { cancelled = true; window.clearTimeout(timer); };
  }, [runtime?.installing, runtime?.version_installing, runtime?.session_running, mods?.applying, busy, watchUntil, refresh]);

  async function runAction(command, args) {
    if (actionLock.current || !runtimeRef.current) return;
    if (command === "set_android_microphone" && (runtimeRef.current.installing || runtimeRef.current.session_running || mods?.applying)) return;
    actionLock.current = command;
    setBusy(command);
    setActionError("");
    setNotice("");
    let unlisten;
    try {
      if (command === "install_android_mods") {
        setModProgress({ detail: "Preparing HQ mods…", progress: -1 });
        unlisten = await listen("android-mod-progress", ({ payload }) => {
          if (active.current && actionLock.current === command) setModProgress(payload);
        });
        progressListener.current = unlisten;
        if (!active.current) return;
      }
      await invoke(command, args);
      if (!active.current) return;
      if (command === "launch_android_game") {
        setNotice("Game launch requested. Check the game screen for its current status.");
        setWatchUntil(Date.now() + 15000);
      } else if (command === "install_android_game") {
        setNotice("Lethal Company setup requested. Sign in to Steam and confirm installation on first use.");
        setWatchUntil(Date.now() + 15000);
      } else if (command === "set_android_microphone") {
        setNotice("Microphone setting saved. It will apply the next time the runtime starts.");
      } else if (command === "install_android_version") {
        setNotice(`Downloading v${args.version} using your Steam account. HQ will return here when installation finishes.`);
        setWatchUntil(Date.now() + 30000);
      }
      await refresh({ reloadMods: command !== "set_android_microphone", afterAction: true });
    } catch (cause) {
      if (active.current) {
        setActionError(errorMessage(cause));
        if (command === "set_android_microphone") await refresh({ reloadMods: false });
      }
    } finally {
      if (unlisten) void unlisten().catch(() => {});
      if (progressListener.current === unlisten) progressListener.current = null;
      actionLock.current = "";
      if (active.current) {
        setBusy("");
        setModProgress(null);
      }
    }
  }

  const sessionActive = Boolean(runtime?.session_running);
  const modApplying = busy === "install_android_mods" || Boolean(mods?.applying);
  const unavailable = checking || !runtime || Boolean(busy) || Boolean(runtime?.installing) || Boolean(runtime?.version_installing);
  const sessionBlocked = unavailable || sessionActive || Boolean(runtime?.phantom_blocked) || modApplying;
  const installerBlocked = unavailable || Boolean(runtime?.phantom_blocked) || modApplying ||
    (sessionActive && !["ready", "suspended"].includes(runtime?.session_phase));
  const modBlocked = sessionBlocked || modsChecking || !mods?.supported || modApplying;
  const microphoneBlocked = unavailable || sessionActive || modApplying;
  const canPlay = !sessionBlocked && !modApplying && runtime?.runtime_installed && runtime?.steam_installed && runtime?.game_installed;
  const installText = runtime?.installing || busy === "install_android_runtime" ? "Installing…" : "Install Linux runtime";
  const displayedMods = (mods?.mods || []).filter(mod => `${mod.name} ${mod.dev}`.toLowerCase().includes(query.trim().toLowerCase()));

  if (checking && !runtime && !statusError) return <HqLoading screen detail="Preparing launcher…" />;

  return (
    <div className="flex h-dvh flex-col bg-[var(--theme-bg)] text-white" data-platform="android" data-layout={compactLayout ? "compact" : "wide"}>
      <header className="flex h-10 shrink-0 items-center gap-2 border-b border-panel-outline px-3">
        <img src="/icon.svg" alt="" className="h-6 w-6" />
        <span className="text-sm font-semibold">HQ Launcher</span>
        <span className="ml-auto text-xs text-white/40">Android</span>
      </header>
      <LauncherWorkspace className="hq-screen-enter">
        <LauncherToolbar>
          <Button type="button" onClick={() => runAction("launch_android_game")} disabled={!canPlay} className="h-11 bg-white px-5 font-[620] text-black hover:bg-white/90">
            {busy === "launch_android_game" ? <HqLogo className="h-4 w-4" /> : <Play className="h-4 w-4" aria-hidden="true" />}HQ Run
          </Button>
          <Select value={String(versions.find(item => item.selected)?.version || mods?.game_version || "")} disabled={sessionBlocked || !runtime?.game_installed || !versions.length} onValueChange={(value) => {
            const version = Number(value), entry = versions.find(item => item.version === version);
            if (entry && !entry.selected) void runAction(entry.installed ? "select_android_version" : "install_android_version", { version });
          }}>
            <SelectTrigger aria-label="Game version" className="h-11 w-auto min-w-24 gap-2 bg-[var(--theme-surface)] px-3"><span>{versions.find(item => item.selected)?.version || mods?.game_version ? `v${versions.find(item => item.selected)?.version || mods.game_version}` : "Version"}</span></SelectTrigger>
            <SelectContent>{versions.map(item => <SelectItem key={item.version} value={String(item.version)}>v{item.version} · {item.installed ? "Installed" : "Download"}</SelectItem>)}</SelectContent>
          </Select>
          {runtime?.runtime_installed && !runtime?.game_installed && <Button type="button" disabled={installerBlocked} onClick={() => runAction("install_android_game")} className="h-11"><Download className="h-4 w-4" aria-hidden="true" />Install Lethal Company</Button>}
          {runtime?.game_installed && <Button type="button" disabled={modBlocked || !mods} onClick={() => runAction("install_android_mods")} title={modApplying ? "Installing HQ mods…" : mods?.installed ? "Reinstall HQ mods" : "Install HQ mods"} className="h-11 w-11 p-0"><Download className="h-4 w-4" aria-hidden="true" /><span className="sr-only">{modApplying ? "Installing HQ mods…" : mods?.installed ? "Reinstall HQ mods" : "Install HQ mods"}</span></Button>}
          <LauncherSearch value={query} onChange={(event) => setQuery(event.target.value)} />
          <Button type="button" onClick={() => { setActionError(""); void refresh(); }} disabled={checking || Boolean(busy)} aria-label="Refresh" className="h-11 w-11 p-0"><RefreshCw className="h-4 w-4" aria-hidden="true" /></Button>
          {sessionActive && <Button type="button" disabled={checking || Boolean(busy)} onClick={() => runAction("stop_android_session")} className="h-11"><Square className="h-4 w-4" aria-hidden="true" />Stop runtime</Button>}
        </LauncherToolbar>

        <div role="status" aria-live="polite" className="shrink-0 break-words text-xs leading-5 text-white/50">
          {checking && !runtime ? "Checking runtime…" : sessionActive ? `Internal runtime active · ${runtime.session_phase}` : runtime?.game_installed ? "Game installed · Runtime ready." : "Install the runtime and Lethal Company in Settings."}
        </div>
        {notice && <p role="status" aria-live="polite" className="shrink-0 break-words text-sm leading-6 text-white/65">{notice}</p>}
        {[statusError, actionError, runtime?.session_error].filter(Boolean).map((message, index) => <p key={`${index}-${message}`} role="alert" className="shrink-0 break-words text-sm leading-6 text-red-300">{message}</p>)}
        {modApplying && <InstallProgress label="HQ mod installation progress" progress={modProgress?.progress} detail={modProgress?.detail || "Installing HQ mods…"} />}
        {runtime?.version_installing && <InstallProgress label="Game version installation progress" progress={-1} detail={runtime.version_install?.stage || "Installing game version…"} />}
        {[versionError, runtime?.version_install?.error].filter(Boolean).map((message, index) => <p key={index} role="alert" className="break-words text-sm text-red-300">{message}</p>)}

        <main data-launcher-panels className="grid min-h-0 flex-1 grid-cols-1 gap-3 overflow-auto min-[800px]:grid-cols-[minmax(0,1fr)_minmax(280px,360px)]">
          <section aria-labelledby="mods-title" className="flex min-h-[240px] flex-col rounded-2xl border border-panel-outline bg-[var(--theme-surface)] p-3 min-[800px]:min-h-0">
            <div className="mb-3 flex items-center justify-between px-1">
              <h1 id="mods-title" className="text-sm font-semibold text-white/80">Mods</h1>
              <span className="text-xs text-white/40">{displayedMods.length} items</span>
            </div>
            <div className="min-h-0 overflow-auto pr-1 max-[799px]:max-h-[55dvh]">
              <div className="flex flex-col gap-2">
                {displayedMods.map((mod) => <LauncherModRow key={`${mod.dev}/${mod.name}`} className={!mod.installed ? "opacity-50" : undefined}>
                  <ModCover initials={`${mod.dev?.[0] || "M"}${mod.name?.[0] || "M"}`.toUpperCase()} />
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-baseline gap-x-2 gap-y-1">
                      <span className="min-w-0 truncate text-base font-semibold">{mod.name}</span>
                      <span className="truncate text-sm text-white/40">{mod.dev}</span>
                    </div>
                    <p className="mt-1 text-sm text-white/50">{mod.installed ? "Installed" : "Not installed"}</p>
                    {mod.blocked_reason && <p className="mt-1 text-xs leading-5 text-white/50">{mod.blocked_reason}</p>}
                  </div>
                  <div className="flex min-h-11 min-w-11 items-center justify-center">
                    <Switch aria-label={`Enable ${mod.name}`} checked={Boolean(mod.enabled)} disabled={modBlocked || !mods.installed || Boolean(mod.blocked_reason)} onCheckedChange={(enabled) => runAction("set_android_mod_enabled", { dev: mod.dev, name: mod.name, enabled })} className="before:absolute before:-inset-y-3 before:inset-x-0" />
                  </div>
                </LauncherModRow>)}
                {!displayedMods.length && <p className="px-2 py-10 text-center text-sm text-white/40">{!runtime?.game_installed ? "Install the game to view mods." : modsChecking ? "Checking HQ mods…" : "No mods found."}</p>}
              </div>
            </div>
          </section>

          <section aria-labelledby="runtime-title" className="min-w-0 rounded-2xl border border-panel-outline bg-[var(--theme-surface)] p-4 min-[800px]:overflow-auto">
            <h2 id="runtime-title" className="mb-3 text-sm font-semibold text-white/80">Settings</h2>
            <div className="space-y-4 text-sm leading-6">
              <div>
                <h3 className="font-medium">Runtime</h3>
                <p className="mt-1 text-xs text-white/50">{runtime?.runtime_installed ? `Internal runtime installed${runtime.runtime_version ? ` · ${runtime.runtime_version}` : ""}` : "Install Linux and Steam inside HQ to run the game."}</p>
                {!runtime?.runtime_installed && <Button type="button" disabled={unavailable || sessionActive} onClick={() => runAction("install_android_runtime")} className="mt-3 min-h-11 w-full"><Download className="h-4 w-4" aria-hidden="true" />{installText}</Button>}
                {runtime?.installing && <InstallProgress label="Runtime installation progress" progress={runtime.install_progress} detail={runtime.install_stage} />}
                {runtime?.install_error && <p role="alert" className="mt-2 break-words text-red-300">{runtime.install_error}</p>}
              </div>
              <div className="border-t border-panel-outline pt-3">
                <h3 className="font-medium">Lethal Company</h3>
                <p className="mt-1 text-xs text-white/50">{runtime?.game_installed ? "Lethal Company installed · Start with HQ Run." : "Use the Install button above to sign in to Steam and install your game."}</p>
                {runtime?.game_installed && <>
                  <p className="mt-2 text-xs text-white/50">{modsChecking ? "Checking HQ mods…" : mods?.game_version ? `Game version ${mods.game_version}${mods.supported ? " · HQ mods supported" : " · HQ mods unsupported"}` : "Game version check required"}</p>
                  {mods?.installed && <p className="mt-1 text-xs text-[var(--theme-accent)]">HQ mods installed</p>}
                  {mods?.reason && <p className="mt-1 break-words text-xs text-white/60">{mods.reason}</p>}
                  {sessionActive && <p className="mt-2 text-xs text-white/50">Stop the runtime before changing mods.</p>}
                  {modError && <p role="alert" className="mt-2 break-words text-red-300">{modError}</p>}
                </>}
              </div>
              <div className="border-t border-panel-outline pt-2">
                <label className={`flex min-h-11 items-center justify-between gap-4 py-2 ${microphoneBlocked ? "opacity-50" : "cursor-pointer"}`}>
                  <span>Enable microphone</span>
                  <input type="checkbox" aria-label="Enable microphone" aria-describedby="microphone-help" checked={Boolean(runtime?.microphone_enabled && runtime?.microphone_permission)} disabled={microphoneBlocked} onChange={(event) => runAction("set_android_microphone", { enabled: event.target.checked })} className="h-5 w-5 shrink-0 accent-[var(--theme-accent)]" />
                </label>
                <p id="microphone-help" className="text-xs text-white/45">Off by default. Enabling this requests Android microphone permission. Stop the runtime before changing this setting. Changes apply the next time it starts.</p>
              </div>
              {runtime?.phantom_blocked && <div className="border-t border-panel-outline pt-3">
                <p className="text-xs text-white/60">Android restricted the runtime. Check Developer settings and try again.</p>
                <Button type="button" disabled={Boolean(busy)} onClick={() => runAction("open_android_developer_settings")} className="mt-3 h-auto min-h-11 w-full whitespace-normal"><Settings className="h-4 w-4 shrink-0" aria-hidden="true" />Open Android Developer settings</Button>
              </div>}
              {runtime?.device && <p className="break-words text-xs text-white/40">{runtime.device} · Android {runtime.android_version}</p>}
            </div>
          </section>
        </main>
      </LauncherWorkspace>
    </div>
  );
}
