import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { CheckCircle2, Download, Play, Search, Settings2 } from "lucide-react";
import { Button } from "./components/ui/button";
import { Input } from "./components/ui/input";
import { Checkbox } from "./components/ui/checkbox";
import { Slider } from "./components/ui/slider";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "./components/ui/select";
import { cn } from "./lib/cn";

function fmtBytes(n) {
  if (typeof n !== "number" || !Number.isFinite(n)) return "";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  const digits = v >= 100 ? 0 : v >= 10 ? 1 : 2;
  return `${v.toFixed(digits)} ${units[i]}`;
}

function modKey(mod) {
  return `${mod.dev}::${mod.name}`;
}

function valueLabel(v) {
  if (!v) return "";
  if (v.type === "Bool") return v.data ? "true" : "false";
  if (v.type === "String") return v.data ?? "";
  if (v.type === "Int") return String(v.data?.value ?? "");
  if (v.type === "Float") return String(v.data?.value ?? "");
  if (v.type === "Enum") return v.data?.options?.[v.data?.index ?? 0] ?? "";
  if (v.type === "Flags") return (v.data?.indicies ?? []).map((i) => v.data?.options?.[i]).filter(Boolean).join(", ");
  return "";
}

export default function App() {
  const [installedVersions, setInstalledVersions] = useState([]);
  const [selectedVersion, setSelectedVersion] = useState(56);
  const [manifest, setManifest] = useState({ version: null, mods: [] });

  const [query, setQuery] = useState("");
  const [selectedMod, setSelectedMod] = useState(null);
  const [modEnabled, setModEnabled] = useState(true);
  const [modToggleBusy, setModToggleBusy] = useState(false);
  const [disabledMods, setDisabledMods] = useState([]); // [{dev,name}] normalized by backend

  // Download confirm modal (for non-installed versions)
  const [downloadPrompt, setDownloadPrompt] = useState({ open: false, version: null });

  // Config editor state (shared config via junction) - BepInEx cfg UI
  const [configFiles, setConfigFiles] = useState([]);
  const [activeConfigPath, setActiveConfigPath] = useState("");
  const [cfgFile, setCfgFile] = useState(null); // parsed FileData
  const [activeSection, setActiveSection] = useState("");
  const [cfgError, setCfgError] = useState("");
  const [savingEntry, setSavingEntry] = useState(null); // `${section}/${entry}`

  // Download/sync progress toast
  const [task, setTask] = useState({
    status: "idle", // idle | working | done | error
    version: null,
    step_name: null,
    steps_total: null,
    step: null,
    overall_percent: null,
    detail: null,
    downloaded_bytes: null,
    total_bytes: null,
    error: null,
  });

  const [gameStatus, setGameStatus] = useState({ running: false, pid: null });

  const isInstalled = useMemo(() => {
    const s = new Set(installedVersions);
    return (v) => s.has(v);
  }, [installedVersions]);

  const filteredMods = useMemo(() => {
    const q = query.trim().toLowerCase();
    const mods = Array.isArray(manifest.mods) ? manifest.mods : [];
    if (!q) return mods;
    return mods.filter((m) => {
      const hay = `${m.dev} ${m.name}`.toLowerCase();
      return hay.includes(q);
    });
  }, [manifest.mods, query]);

  const progressText = useMemo(() => {
    const p = task.overall_percent;
    if (typeof p === "number") return `${p.toFixed(1)}%`;
    return "";
  }, [task.overall_percent]);

  const statusText = useMemo(() => {
    const base =
      task.status === "error" ? "Error" : task.status === "done" ? "Done" : "Working";
    const v = task.version != null ? ` v${task.version}` : "";
    const step =
      task.steps_total && task.step ? ` • Step ${task.step}/${task.steps_total}` : "";
    const name = task.step_name ? ` • ${task.step_name}` : "";
    return `${base}${v}${step}${name}`.trim();
  }, [task.status, task.step, task.step_name, task.steps_total, task.version]);

  const bytesText = useMemo(() => {
    if (typeof task.downloaded_bytes !== "number") return "";
    const d = fmtBytes(task.downloaded_bytes);
    if (typeof task.total_bytes === "number") return `${d} / ${fmtBytes(task.total_bytes)}`;
    return d;
  }, [task.downloaded_bytes, task.total_bytes]);

  // bootstrap data
  useEffect(() => {
    (async () => {
      const [versions, mf] = await Promise.all([
        invoke("list_installed_versions"),
        invoke("get_manifest"),
      ]);
      setInstalledVersions(Array.isArray(versions) ? versions : []);
      setManifest(mf ?? { version: null, mods: [] });

      // pick best default selected version
      const vList = Array.isArray(versions) ? versions : [];
      if (vList.length > 0) setSelectedVersion(vList[vList.length - 1]);

      // initial running status
      try {
        const s = await invoke("get_game_status");
        setGameStatus(s ?? { running: false, pid: null });
      } catch {}

      // disabled mods list
      try {
        const dm = await invoke("get_disabled_mods");
        setDisabledMods(Array.isArray(dm) ? dm : []);
      } catch {}
    })().catch((e) => {
      console.error(e);
    });
  }, []);

  const disabledSet = useMemo(() => {
    const s = new Set();
    for (const m of disabledMods) {
      if (!m) continue;
      s.add(`${String(m.dev).toLowerCase()}::${String(m.name).toLowerCase()}`);
    }
    return s;
  }, [disabledMods]);

  // poll game status while running (detect exit)
  useEffect(() => {
    if (!gameStatus.running) return;
    const t = setInterval(() => {
      invoke("get_game_status")
        .then((s) => setGameStatus(s ?? { running: false, pid: null }))
        .catch(() => {});
    }, 1500);
    return () => clearInterval(t);
  }, [gameStatus.running]);

  // listen to backend progress events
  useEffect(() => {
    let unlistenProgress = null;
    let unlistenFinished = null;
    let unlistenError = null;

    (async () => {
      unlistenProgress = await listen("download://progress", (event) => {
        setTask((t) => ({
          ...t,
          status: "working",
          ...event.payload,
          error: null,
        }));
      });
      unlistenFinished = await listen("download://finished", (event) => {
        setTask((t) => ({
          ...t,
          status: "done",
          ...event.payload,
        }));
        // refresh installed versions list after install
        invoke("list_installed_versions")
          .then((v) => setInstalledVersions(Array.isArray(v) ? v : []))
          .catch(() => {});
      });
      unlistenError = await listen("download://error", (event) => {
        setTask((t) => ({
          ...t,
          status: "error",
          version: event.payload?.version ?? t.version,
          error: event.payload?.message ?? "Unknown error",
        }));
      });
    })();

    return () => {
      if (typeof unlistenProgress === "function") unlistenProgress();
      if (typeof unlistenFinished === "function") unlistenFinished();
      if (typeof unlistenError === "function") unlistenError();
    };
  }, []);

  // when selecting a mod, load candidate config files
  useEffect(() => {
    if (!selectedMod) {
      setConfigFiles([]);
      setActiveConfigPath("");
      setCfgFile(null);
      setActiveSection("");
      setCfgError("");
      setModEnabled(true);
      return;
    }

    (async () => {
      // enabled state is global (disablemod file) and applies to all versions
      const key = `${String(selectedMod.dev).toLowerCase()}::${String(selectedMod.name).toLowerCase()}`;
      setModEnabled(!disabledSet.has(key));

      const files = await invoke("list_config_files_for_mod", {
        dev: selectedMod.dev,
        name: selectedMod.name,
      });
      const list = (Array.isArray(files) ? files : []).filter((p) =>
        String(p).toLowerCase().endsWith(".cfg"),
      );
      setConfigFiles(list);
      const next = list[0] ?? "";
      setActiveConfigPath(next);
    })().catch((e) => console.error(e));
  }, [selectedMod, selectedVersion]);

  // load + parse active cfg file
  useEffect(() => {
    if (!activeConfigPath) {
      setCfgFile(null);
      setActiveSection("");
      setCfgError("");
      return;
    }

    (async () => {
      setCfgError("");
      const parsed = await invoke("read_bepinex_cfg", { relPath: activeConfigPath });
      setCfgFile(parsed ?? null);
      const firstSection = parsed?.sections?.[0]?.name ?? "";
      setActiveSection(firstSection);
    })().catch((e) => {
      console.error(e);
      setCfgFile(null);
      setActiveSection("");
      setCfgError(e?.message ?? String(e));
    });
  }, [activeConfigPath]);

  async function downloadVersion(v) {
    setTask((t) => ({
      ...t,
      status: "working",
      version: v,
      overall_percent: 0,
      error: null,
    }));
    try {
      await invoke("download", { version: v });
    } catch (e) {
      // Backend also emits download://error, but ensure UI reacts if invoke fails early.
      setTask((t) => ({
        ...t,
        status: "error",
        version: v,
        error: e?.message ?? String(e),
      }));
    }
  }

  function openDownloadPrompt(v) {
    setDownloadPrompt({ open: true, version: v });
  }

  async function setCfgEntry(sectionName, entryName, nextValue) {
    if (!activeConfigPath) return;
    const key = `${sectionName}/${entryName}`;
    setSavingEntry(key);
    // optimistic update
    setCfgFile((f) => {
      if (!f) return f;
      const next = {
        ...f,
        sections: (f.sections ?? []).map((s) => {
          if (s.name !== sectionName) return s;
          return {
            ...s,
            entries: (s.entries ?? []).map((e) =>
              e.name === entryName ? { ...e, value: nextValue } : e,
            ),
          };
        }),
      };
      return next;
    });

    try {
      await invoke("set_bepinex_cfg_entry", {
        args: {
          rel_path: activeConfigPath,
          section: sectionName,
          entry: entryName,
          value: nextValue,
        },
      });
    } catch (e) {
      console.error(e);
      setCfgError(e?.message ?? String(e));
      // re-parse to resync
      try {
        const parsed = await invoke("read_bepinex_cfg", { relPath: activeConfigPath });
        setCfgFile(parsed ?? null);
      } catch {}
    } finally {
      setSavingEntry(null);
    }
  }

  async function toggleModEnabled(nextEnabled) {
    if (!selectedMod) return;
    if (!isInstalled(selectedVersion)) {
      openDownloadPrompt(selectedVersion);
      return;
    }
    setModToggleBusy(true);
    try {
      await invoke("set_mod_enabled", {
        version: selectedVersion,
        dev: selectedMod.dev,
        name: selectedMod.name,
        enabled: !!nextEnabled,
      });
      // refresh disabled list (source of truth)
      const dm = await invoke("get_disabled_mods");
      setDisabledMods(Array.isArray(dm) ? dm : []);
      setModEnabled(!!nextEnabled);
    } catch (e) {
      console.error(e);
      setCfgError(e?.message ?? String(e));
    } finally {
      setModToggleBusy(false);
    }
  }

  const versionOptions = useMemo(() => {
    const set = new Set(installedVersions);
    set.add(selectedVersion);
    // small suggested list (so there are “not installed” entries too)
    [40, 49, 50, 56, 62, 64, 69, 72, 73].forEach((v) => set.add(v));
    return Array.from(set).sort((a, b) => b - a);
  }, [installedVersions, selectedVersion]);

  const selectedInstalled = isInstalled(selectedVersion);
  const promptVersion = downloadPrompt.version;
  const promptIsWorking =
    downloadPrompt.open && task.status === "working" && task.version === promptVersion;
  const promptIsDone =
    downloadPrompt.open && task.status === "done" && task.version === promptVersion;
  const promptIsError =
    downloadPrompt.open && task.status === "error" && task.version === promptVersion;

  async function startRun() {
    if (gameStatus.running) {
      try {
        await invoke("stop_game");
      } finally {
        setGameStatus({ running: false, pid: null });
      }
      return;
    }
    if (!isInstalled(selectedVersion)) {
      openDownloadPrompt(selectedVersion);
      return;
    }
    try {
      const pid = await invoke("launch_game", { version: selectedVersion });
      setGameStatus({ running: true, pid: typeof pid === "number" ? pid : null });
    } catch (e) {
      console.error(e);
      // Reuse the download modal error area for now
      setTask((t) => ({
        ...t,
        status: "error",
        version: selectedVersion,
        error: e?.message ?? String(e),
      }));
    }
  }

  return (
    <div className="h-full text-white">
      <div className="mx-auto flex h-full max-w-[1600px] flex-col gap-4 p-4">
        {/* Top bar */}
        <div className="flex items-center gap-3">
          <Button
            variant={gameStatus.running ? "secondary" : "default"}
            className="h-11 px-5"
            onClick={startRun}
          >
            <Play className="h-4 w-4" />
            {gameStatus.running ? "Stop" : "Start Run"}
          </Button>

          <div className="w-fit">
            <Select
              value={String(selectedVersion)}
              onValueChange={(v) => {
                const nextV = Number(v);
                if (isInstalled(nextV)) {
                  setSelectedVersion(nextV);
                  // Apply global disablemod list to the selected version.
                  invoke("apply_disabled_mods", { version: nextV }).catch(() => {});
                } else {
                  openDownloadPrompt(nextV);
                }
              }}
            >
              <SelectTrigger className="h-11 px-3">
                <div className="flex items-center gap-2 mr-2">
                  <div className="text-sm font-semibold">v{selectedVersion}</div>
                  {selectedInstalled ? (
                    <CheckCircle2 className="h-4 w-4 text-emerald-400" />
                  ) : (
                    <Download className="h-4 w-4 text-amber-300" />
                  )}
                </div>
                {/* <SelectValue /> */}
              </SelectTrigger>
              <SelectContent>
                {versionOptions.map((v) => (
                  <SelectItem
                    key={v}
                    value={String(v)}
                    marker={
                      isInstalled(v) ? null : <Download className="h-4 w-4 text-amber-300" />
                    }
                  >
                    <span className="inline-flex items-center gap-2">
                      <span>v{v}</span>
                      {isInstalled(v) ? (
                        <span className="text-white/45">(installed)</span>
                      ) : (
                        <span className="text-white/45">(download)</span>
                      )}
                    </span>
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          {!selectedInstalled && (
            <Button
              variant="secondary"
              className="h-11"
              onClick={() => openDownloadPrompt(selectedVersion)}
            >
              <Download className="h-4 w-4" />
              Download
            </Button>
          )}

          <div className="relative ml-2 flex-1">
            <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-white/40" />
            <Input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search Mod"
              className="h-11 pl-10"
            />
          </div>
        </div>

        {/* Main grid */}
        <div className="grid min-h-0 flex-1 grid-cols-[repeat(auto-fit,minmax(0,1fr))] gap-4">
          {/* Mod list */}
          <div className="min-h-0 rounded-2xl border border-white/10 bg-white/5 p-3">
            <div className="mb-3 flex items-center justify-between px-1">
              <div className="text-sm font-semibold text-white/80">
                Mods {manifest.version != null ? `(manifest v${manifest.version})` : ""}
              </div>
              <div className="text-xs text-white/40">{filteredMods.length} items</div>
            </div>

            <div className="h-[calc(100%-2.25rem)] overflow-auto pr-1">
              <div className="flex flex-col gap-2">
                {filteredMods.map((m) => {
                  const selected = selectedMod && modKey(selectedMod) === modKey(m);
                  const initials = `${m.dev?.[0] ?? "M"}${m.name?.[0] ?? "M"}`.toUpperCase();
                  return (
                    <button
                      key={modKey(m)}
                      className={cn(
                        "group flex w-full items-start gap-3 rounded-2xl border px-3 py-3 text-left transition",
                        selected
                          ? "border-white/20 bg-white/10"
                          : "border-white/10 bg-black/10 hover:bg-white/10",
                      )}
                      onClick={() => setSelectedMod(m)}
                    >
                      <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl bg-white/10 text-sm font-bold text-white/80">
                        {initials}
                      </div>
                      <div className="min-w-0 flex-1">
                        <div className="flex items-baseline gap-2">
                          <div className="truncate text-base font-semibold">{m.name}</div>
                          <div className="truncate text-sm text-white/40">{m.dev}</div>
                        </div>
                        <div className="mt-1 line-clamp-1 text-sm text-white/50">
                          Click to edit config
                        </div>
                      </div>
                      <Settings2 className="mt-1 h-4 w-4 shrink-0 text-white/30 opacity-0 transition group-hover:opacity-100" />
                    </button>
                  );
                })}
                {filteredMods.length === 0 && (
                  <div className="px-2 py-10 text-center text-sm text-white/40">
                    No mods found.
                  </div>
                )}
              </div>
            </div>
          </div>

          {/* Right panel: config editor */}
          {!selectedMod ? (
            <></>
          ) : (
          
          <div className="min-h-0 rounded-2xl border border-white/10 bg-white/5 p-4">
              <div className="flex h-full flex-col gap-3">
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <div className="truncate text-lg font-semibold">{selectedMod.name}</div>
                    <div className="truncate text-sm text-white/40">{selectedMod.dev}</div>
                  </div>
                  <Button
                    variant="secondary"
                    size="sm"
                    onClick={() => {
                      setSelectedMod(null);
                    }}
                  >
                    Close
                  </Button>
                </div>

                <div className="flex items-center justify-between rounded-2xl border border-white/10 bg-black/10 px-3 py-2">
                  <div className="text-sm font-semibold text-white/80">Enabled</div>
                  <div className="flex items-center gap-2">
                    <div className="text-xs text-white/50">
                      {modToggleBusy ? "Applying..." : modEnabled ? "On" : "Off"}
                    </div>
                    <Checkbox
                      checked={modEnabled}
                      disabled={modToggleBusy}
                      onCheckedChange={(v) => toggleModEnabled(!!v)}
                    />
                  </div>
                </div>

                <div className="flex items-center gap-2">
                  <div className="text-xs font-semibold text-white/50">Section</div>
                  <div className="flex-1">
                    <Select
                      value={activeSection}
                      onValueChange={(v) => setActiveSection(v)}
                      disabled={!cfgFile || (cfgFile.sections ?? []).length === 0}
                    >
                      <SelectTrigger className="h-9">
                        <SelectValue placeholder="(no sections)" />
                      </SelectTrigger>
                      <SelectContent>
                        {(cfgFile?.sections ?? []).map((s) => (
                          <SelectItem key={s.name} value={s.name}>
                            {s.name || "(nameless)"}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                </div>

                {activeConfigPath && (
                  <div className="px-1 text-[11px] text-white/35">
                    File: <span className="text-white/50">{activeConfigPath}</span>
                  </div>
                )}

                {cfgError && (
                  <div className="rounded-xl border border-red-400/20 bg-red-400/10 px-3 py-2 text-xs text-red-200">
                    {cfgError}
                  </div>
                )}

                {!activeConfigPath ? (
                  <div className="flex flex-1 items-center justify-center text-sm text-white/40">
                    No config file matched this mod yet.
                  </div>
                ) : !cfgFile ? (
                  <div className="flex flex-1 items-center justify-center text-sm text-white/40">
                    Loading cfg...
                  </div>
                ) : (
                  <div className="min-h-0 flex flex-1 overflow-hidden">
                    <div className="min-h-0 flex-1 overflow-auto rounded-2xl border border-white/10 bg-black/10 p-3">
                      {(() => {
                        const s = (cfgFile.sections ?? []).find((x) => x.name === activeSection) ??
                          cfgFile.sections?.[0];
                        if (!s) return null;
                        return (
                          <div className="flex flex-col gap-3">
                            {(s.entries ?? []).map((e) => {
                              const id = `${s.name}/${e.name}`;
                              const v = e.value;
                              return (
                                <div
                                  key={id}
                                  className="rounded-2xl border border-white/10 bg-white/5 p-3"
                                >
                                  <div className="flex items-start justify-between gap-3">
                                    <div className="min-w-0">
                                      <div className="truncate text-sm font-semibold">{e.name}</div>
                                      {e.description && (
                                        <div className="mt-1 whitespace-pre-wrap text-xs text-white/50">
                                          {e.description}
                                        </div>
                                      )}
                                    </div>
                                    <div className="shrink-0 text-[10px] text-white/40">
                                      {savingEntry === id ? "Saving..." : v?.type ?? ""}
                                    </div>
                                  </div>

                                  <div className="mt-3">
                                    {v?.type === "Bool" ? (
                                      <label className="flex cursor-pointer items-center gap-2 text-sm">
                                        <Checkbox
                                          checked={!!v.data}
                                          onCheckedChange={(checked) =>
                                            setCfgEntry(s.name, e.name, {
                                              type: "Bool",
                                              data: !!checked,
                                            })
                                          }
                                        />
                                        <span className="text-white/80">Enabled</span>
                                      </label>
                                    ) : v?.type === "Int" ? (
                                      v.data?.range ? (
                                        <div className="flex items-center gap-3">
                                          <div className="w-20 shrink-0 text-xs text-white/50">
                                            {v.data.range.start}–{v.data.range.end}
                                          </div>
                                          <Slider
                                            value={[v.data.value ?? 0]}
                                            min={v.data.range.start}
                                            max={v.data.range.end}
                                            step={1}
                                            onValueChange={([val]) =>
                                              setCfgEntry(s.name, e.name, {
                                                type: "Int",
                                                data: {
                                                  value: val,
                                                  range: v.data?.range ?? null,
                                                },
                                              })
                                            }
                                          />
                                          <div className="w-16 shrink-0 text-right text-sm text-white/80 tabular-nums">
                                            {v.data.value ?? 0}
                                          </div>
                                        </div>
                                      ) : (
                                        <Input
                                          type="number"
                                          value={v.data?.value ?? 0}
                                          onChange={(ev) =>
                                            setCfgEntry(s.name, e.name, {
                                              type: "Int",
                                              data: {
                                                value: Number(ev.target.value),
                                                range: v.data?.range ?? null,
                                              },
                                            })
                                          }
                                        />
                                      )
                                    ) : v?.type === "Float" ? (
                                      v.data?.range ? (
                                        <div className="flex items-center gap-3">
                                          <div className="w-24 shrink-0 text-xs text-white/50">
                                            {v.data.range.start}–{v.data.range.end}
                                          </div>
                                          <Slider
                                            value={[v.data.value ?? 0]}
                                            min={v.data.range.start}
                                            max={v.data.range.end}
                                            step={(v.data.range.end - v.data.range.start) / 200 || 0.01}
                                            onValueChange={([val]) =>
                                              setCfgEntry(s.name, e.name, {
                                                type: "Float",
                                                data: {
                                                  value: val,
                                                  range: v.data?.range ?? null,
                                                },
                                              })
                                            }
                                          />
                                          <div className="w-20 shrink-0 text-right text-sm text-white/80 tabular-nums">
                                            {Number(v.data.value ?? 0).toFixed(3)}
                                          </div>
                                        </div>
                                      ) : (
                                        <Input
                                          type="number"
                                          step="any"
                                          value={v.data?.value ?? 0}
                                          onChange={(ev) =>
                                            setCfgEntry(s.name, e.name, {
                                              type: "Float",
                                              data: {
                                                value: Number(ev.target.value),
                                                range: v.data?.range ?? null,
                                              },
                                            })
                                          }
                                        />
                                      )
                                    ) : v?.type === "Enum" ? (
                                      <Select
                                        value={String(v.data?.index ?? 0)}
                                        onValueChange={(val) =>
                                          setCfgEntry(s.name, e.name, {
                                            type: "Enum",
                                            data: {
                                              index: Number(val),
                                              options: v.data?.options ?? [],
                                            },
                                          })
                                        }
                                      >
                                        <SelectTrigger>
                                          <SelectValue placeholder="Select..." />
                                        </SelectTrigger>
                                        <SelectContent>
                                          {(v.data?.options ?? []).map((opt, idx) => (
                                            <SelectItem key={opt} value={String(idx)}>
                                              {opt}
                                            </SelectItem>
                                          ))}
                                        </SelectContent>
                                      </Select>
                                    ) : v?.type === "Flags" ? (
                                      <div className="flex flex-col gap-2">
                                        {(v.data?.options ?? []).map((opt, idx) => {
                                          const checked = (v.data?.indicies ?? []).includes(idx);
                                          return (
                                            <label
                                              key={opt}
                                              className="flex cursor-pointer items-center gap-2 text-sm text-white/80"
                                            >
                                              <Checkbox
                                                checked={checked}
                                                onCheckedChange={(nextChecked) => {
                                                  const set = new Set(v.data?.indicies ?? []);
                                                  if (nextChecked) set.add(idx);
                                                  else set.delete(idx);
                                                  setCfgEntry(s.name, e.name, {
                                                    type: "Flags",
                                                    data: {
                                                      indicies: Array.from(set).sort((a, b) => a - b),
                                                      options: v.data?.options ?? [],
                                                    },
                                                  });
                                                }}
                                              />
                                              <span>{opt}</span>
                                            </label>
                                          );
                                        })}
                                      </div>
                                    ) : (
                                      <Input
                                        value={valueLabel(v)}
                                        onChange={(ev) =>
                                          setCfgEntry(s.name, e.name, {
                                            type: "String",
                                            data: ev.target.value,
                                          })
                                        }
                                      />
                                    )}
                                  </div>
                                </div>
                              );
                            })}
                          </div>
                        );
                      })()}
                    </div>
                  </div>
                )}
              </div>
          </div>
          
          )}
        </div>

      </div>

      {/* Download confirm modal */}
      {downloadPrompt.open && (
        <div className="fixed inset-0 z-50 flex items-center justify-center">
          <button
            className="absolute inset-0 bg-black/60"
            onClick={() => {
              if (!promptIsWorking) setDownloadPrompt({ open: false, version: null });
            }}
            aria-label="Close"
          />

          <div className="relative w-[min(520px,calc(100vw-2rem))] rounded-2xl border border-white/10 bg-[#0f1116] p-5 shadow-2xl shadow-black/50">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <div className="text-lg font-semibold">
                  Download v{promptVersion}?
                </div>
                <div className="mt-1 text-sm text-white/55">
                  This version is not downloaded yet. Do you want to download it now?
                </div>
              </div>
            </div>

            {/* Progress (from Rust emit) */}
            {(promptIsWorking || promptIsDone || promptIsError) && (
              <div className="mt-4 rounded-2xl border border-white/10 bg-white/5 p-3">
                <div className="flex items-center justify-between gap-3">
                  <div className="min-w-0">
                    <div className="truncate text-sm font-semibold">{statusText}</div>
                    <div className="truncate text-xs text-white/50">
                      {task.detail || (bytesText ? `Downloaded: ${bytesText}` : "")}
                    </div>
                    {task.error && <div className="mt-1 text-xs text-red-300">{task.error}</div>}
                  </div>
                  <div className="shrink-0 text-sm text-white/70">{progressText}</div>
                </div>
                <div className="mt-2 h-2 w-full overflow-hidden rounded-full bg-white/10">
                  <div
                    className={cn(
                      "h-full rounded-full transition-[width]",
                      task.status === "error" ? "bg-red-400" : "bg-emerald-400",
                    )}
                    style={{ width: `${Math.max(0, Math.min(100, task.overall_percent ?? 0))}%` }}
                  />
                </div>
              </div>
            )}

            <div className="mt-5 flex items-center justify-end gap-2">
              {promptIsWorking ? (
                <Button variant="default" disabled className="h-10 min-w-[120px]">
                  <Download className="h-4 w-4" />
                  Downloading...
                </Button>
              ) : promptIsDone ? (
                <Button
                  variant="default"
                  className="h-10 min-w-[120px]"
                  onClick={() => setDownloadPrompt({ open: false, version: null })}
                >
                  Close
                </Button>
              ) : promptIsError ? (
                <Button
                  variant="default"
                  className="h-10 min-w-[120px]"
                  onClick={() => setDownloadPrompt({ open: false, version: null })}
                >
                  Close
                </Button>
              ) : (
                <>
                  <Button
                    variant="secondary"
                    onClick={() => setDownloadPrompt({ open: false, version: null })}
                    className="h-10"
                  >
                    Cancel
                  </Button>
                  <Button
                    variant="default"
                    onClick={() => {
                      if (typeof promptVersion !== "number") return;
                      setSelectedVersion(promptVersion);
                      downloadVersion(promptVersion);
                    }}
                    disabled={typeof promptVersion !== "number"}
                    className="h-10"
                  >
                    <Download className="h-4 w-4" />
                    Download
                  </Button>
                </>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
 }
