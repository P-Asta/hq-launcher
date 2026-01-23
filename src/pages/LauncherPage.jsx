import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  CheckCircle2,
  ChevronDown,
  Download,
  LogOut,
  Play,
  Search,
  Settings2,
} from "lucide-react";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";
import { Checkbox } from "../components/ui/checkbox";
import { Switch } from "../components/ui/switch";
import { Slider } from "../components/ui/slider";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../components/ui/select";
import { cn } from "../lib/cn";

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
  if (v.type === "Flags")
    return (v.data?.indicies ?? [])
      .map((i) => v.data?.options?.[i])
      .filter(Boolean)
      .join(", ");
  return "";
}

function isAuthError(e) {
  const msg = e?.message ?? String(e ?? "");
  const m = msg.toLowerCase();
  return (
    m.includes("not logged in") ||
    m.includes("two-factor") ||
    m.includes("steam guard") ||
    m.includes("missing username for remembered login")
  );
}

export default function LauncherPage({
  loginState,
  onLogout,
  onRequireLogin,
  bootstrapError,
}) {
  const [installedVersions, setInstalledVersions] = useState([]);
  const [selectedVersion, setSelectedVersion] = useState(56);
  const [manifest, setManifest] = useState({
    version: null,
    mods: [],
    manifests: {},
  });

  const [query, setQuery] = useState("");
  const [selectedMod, setSelectedMod] = useState(null);
  const [modEnabled, setModEnabled] = useState(true);
  const [modToggleBusy, setModToggleBusy] = useState(false);
  const [modToggleBusyKeys, setModToggleBusyKeys] = useState(() => new Set());
  const [disabledMods, setDisabledMods] = useState([]); // [{dev,name}] normalized by backend
  const [installedModVersionsByVersion, setInstalledModVersionsByVersion] =
    useState({}); // version -> { key(dev::name lower) -> version }
  const [modCfgFilesByKey, setModCfgFilesByKey] = useState({}); // key(dev::name lower) -> ["foo.cfg", ...]

  // Download confirm modal (for non-installed versions)
  const [downloadPrompt, setDownloadPrompt] = useState({
    open: false,
    version: null,
  });

  const [updatePrompt, setUpdatePrompt] = useState({ open: false });
  const [practicePrompt, setPracticePrompt] = useState({ open: false });
  const [practiceTask, setPracticeTask] = useState(null); // last Practice Mods progress payload

  const [checkUpdatePrompt, setCheckUpdatePrompt] = useState({
    open: false,
    mods: [],
  });

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

  const [checkUpdateTask, setCheckUpdateTask] = useState({
    status: "idle", // idle | working | done | error
    version: null,
    step_name: null,
    steps_total: null,
    step: null,
    overall_percent: null,
    detail: null,
    updatable_mods: [],
    checked: 0,
    total: 0,
  });

  const [gameStatus, setGameStatus] = useState({ running: false, pid: null });
  const [runMode, setRunMode] = useState("normal"); // normal | practice
  const autoCheckedRef = useRef(new Set());

  const isInstalled = useMemo(() => {
    const s = new Set(installedVersions);
    return (v) => s.has(v);
  }, [installedVersions]);

  const installedModVersions = useMemo(() => {
    const v = Number(selectedVersion);
    if (!Number.isFinite(v)) return {};
    const byV = installedModVersionsByVersion?.[v];
    return byV && typeof byV === "object" ? byV : {};
  }, [installedModVersionsByVersion, selectedVersion]);

  const filteredMods = useMemo(() => {
    const q = query.trim().toLowerCase();
    const mods = Array.isArray(manifest.mods) ? manifest.mods : [];

    const selectedKeyLower = selectedMod
      ? `${String(selectedMod.dev).toLowerCase()}::${String(
          selectedMod.name
        ).toLowerCase()}`
      : null;

    const chainConfigs = Array.isArray(manifest.chain_config)
      ? manifest.chain_config
      : [];

    const canonicalChainId = (paths) =>
      (Array.isArray(paths) ? paths : []).slice().sort().join("|");

    const chainIdForMod = (m) => {
      const devLower = String(m?.dev ?? "").toLowerCase();
      const nameLower = String(m?.name ?? "").toLowerCase();
      const keyLower = `${devLower}::${nameLower}`;

      // Prefer confirmed mapping (from backend list_config_files_for_mod)
      const cfgs = modCfgFilesByKey[keyLower];
      if (Array.isArray(cfgs) && cfgs.length > 0) {
        for (const cfgPath of cfgs) {
          const chain = chainConfigs.find((paths) => paths.includes(cfgPath));
          if (Array.isArray(chain) && chain.length > 0) return canonicalChainId(chain);
        }
      }

      // Fallback: best-effort inference by substring match against chain paths
      // (same heuristic the backend uses for list_config_files_for_mod filtering).
      for (const chain of chainConfigs) {
        if (!Array.isArray(chain) || chain.length === 0) continue;
        const hit = chain.some((p) => {
          const lp = String(p ?? "").toLowerCase();
          if (!lp.endsWith(".cfg")) return false;
          return (devLower && lp.includes(devLower)) || (nameLower && lp.includes(nameLower));
        });
        if (hit) return canonicalChainId(chain);
      }
      return null;
    };

    const matchesQuery = (m) => {
      // Preserve existing special-case exclusion
      if (String(m?.name ?? "") === "ShipLootCruiser") return false;
      if (!q) return true;
      const hay = `${m.dev} ${m.name}`.toLowerCase();
      return hay.includes(q);
    };

    // 1) Build chain groups using ALL mods (so search can match any member,
    // but representative can still prefer an installed member even if it doesn't match the query).
    const groups = new Map(); // chainId -> [mods]
    const noChain = [];
    for (const m of mods) {
      const id = chainIdForMod(m);
      if (!id) {
        noChain.push(m);
        continue;
      }
      const arr = groups.get(id) ?? [];
      arr.push(m);
      groups.set(id, arr);
    }

    // 2) Choose representatives per group (prefer selected, then installed for this version)
    const repsByChainId = new Map();
    for (const [id, list] of groups.entries()) {
      if (selectedKeyLower) {
        const selectedInGroup = list.find((m) => {
          const k = `${String(m.dev).toLowerCase()}::${String(m.name).toLowerCase()}`;
          return k === selectedKeyLower;
        });
        if (selectedInGroup) {
          repsByChainId.set(id, selectedInGroup);
          continue;
        }
      }

      // Prefer installed mods as the representative when showing chained groups.
      const installedInGroup = list.find((m) => {
        const k = `${String(m.dev).toLowerCase()}::${String(m.name).toLowerCase()}`;
        return !!installedModVersions[k];
      });
      if (installedInGroup) {
        repsByChainId.set(id, installedInGroup);
        continue;
      }

      repsByChainId.set(id, list[0]);
    }

    // 3) Apply query filtering:
    // - noChain mods: match normally
    // - chain groups: match if ANY member matches, but show representative
    const includedChainIds = new Set();
    for (const [id, list] of groups.entries()) {
      if (!q) {
        includedChainIds.add(id);
        continue;
      }
      if (list.some(matchesQuery)) includedChainIds.add(id);
    }

    // Preserve ordering based on original `mods` ordering.
    const out = [];
    const added = new Set();
    for (const m of mods) {
      const id = chainIdForMod(m);
      const keyLower = `${String(m.dev).toLowerCase()}::${String(m.name).toLowerCase()}`;
      if (!id) {
        if (!matchesQuery(m)) continue;
        if (!added.has(keyLower)) {
          out.push(m);
          added.add(keyLower);
        }
        continue;
      }
      if (!includedChainIds.has(id)) continue;
      const rep = repsByChainId.get(id);
      if (!rep) continue;
      const repKeyLower = `${String(rep.dev).toLowerCase()}::${String(
        rep.name
      ).toLowerCase()}`;
      if (!added.has(repKeyLower)) {
        out.push(rep);
        added.add(repKeyLower);
      }
    }

    return out;
  }, [
    manifest.mods,
    manifest.chain_config,
    query,
    selectedMod,
    modCfgFilesByKey,
    installedModVersions,
  ]);

  // Best-effort prefetch of config-file matches per mod so chain-dedup is accurate.
  useEffect(() => {
    const mods = Array.isArray(manifest.mods) ? manifest.mods : [];
    if (mods.length === 0) return;

    let cancelled = false;
    (async () => {
      // Fetch in small batches to avoid spamming the backend.
      const CONCURRENCY = 6;
      const queue = mods
        .map((m) => ({
          mod: m,
          keyLower: `${String(m.dev).toLowerCase()}::${String(m.name).toLowerCase()}`,
        }))
        .filter((x) => x.keyLower && modCfgFilesByKey[x.keyLower] == null);

      if (queue.length === 0) return;

      const nextMap = { ...modCfgFilesByKey };
      let idx = 0;
      async function worker() {
        while (idx < queue.length && !cancelled) {
          const cur = queue[idx++];
          try {
            const files = await invoke("list_config_files_for_mod", {
              dev: cur.mod.dev,
              name: cur.mod.name,
            });
            nextMap[cur.keyLower] = (Array.isArray(files) ? files : [])
              .map((p) => String(p))
              .filter((p) => p.toLowerCase().endsWith(".cfg"));
          } catch {
            nextMap[cur.keyLower] = [];
          }
        }
      }

      await Promise.all(Array.from({ length: CONCURRENCY }, () => worker()));
      if (!cancelled) setModCfgFilesByKey(nextMap);
    })();

    return () => {
      cancelled = true;
    };
    // Intentionally not including modCfgFilesByKey in deps to avoid infinite re-fetch loops.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [manifest.mods]);

  const progressText = useMemo(() => {
    const p = task.overall_percent;
    if (typeof p === "number") return `${p.toFixed(1)}%`;
    return "";
  }, [task.overall_percent]);

  const statusText = useMemo(() => {
    const base =
      task.status === "error"
        ? "Error"
        : task.status === "done"
        ? "Done"
        : "Working";
    const v = task.version != null ? ` v${task.version}` : "";
    const step =
      task.steps_total && task.step
        ? ` • Step ${task.step}/${task.steps_total}`
        : "";
    const name = task.step_name ? ` • ${task.step_name}` : "";
    return `${base}${v}${step}${name}`.trim();
  }, [task.status, task.step, task.step_name, task.steps_total, task.version]);

  const bytesText = useMemo(() => {
    if (typeof task.downloaded_bytes !== "number") return "";
    const d = fmtBytes(task.downloaded_bytes);
    if (typeof task.total_bytes === "number")
      return `${d} / ${fmtBytes(task.total_bytes)}`;
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
      setManifest(mf ?? { version: null, mods: [], manifests: {} });

      // pick best default selected version
      const vList = Array.isArray(versions) ? versions : [];
      const remoteV =
        mf?.manifests && typeof mf.manifests === "object"
          ? Object.keys(mf.manifests)
              .map((k) => Number(k))
              .filter((n) => Number.isFinite(n))
          : [];
      remoteV.sort((a, b) => a - b);
      if (vList.length > 0) setSelectedVersion(vList[vList.length - 1]);
      else if (remoteV.length > 0)
        setSelectedVersion(remoteV[remoteV.length - 1]);

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

  async function refreshInstalledModVersions(v = selectedVersion) {
    const vv = Number(v);
    if (!Number.isFinite(vv)) return;
    if (!isInstalled(v)) {
      setInstalledModVersionsByVersion((prev) => ({ ...prev, [vv]: {} }));
      return;
    }
    try {
      const list = await invoke("list_installed_mod_versions", { version: v });
      const map = {};
      for (const it of Array.isArray(list) ? list : []) {
        const k = `${String(it.dev).toLowerCase()}::${String(
          it.name
        ).toLowerCase()}`;
        map[k] = String(it.version ?? "");
      }
      setInstalledModVersionsByVersion((prev) => ({ ...prev, [vv]: map }));
    } catch {
      // best-effort (missing folder, etc)
      setInstalledModVersionsByVersion((prev) => ({ ...prev, [vv]: {} }));
    }
  }

  // auto-run update check once on startup for installed selected version
  useEffect(() => {
    if (!isInstalled(selectedVersion)) return;
    if (autoCheckedRef.current.has(selectedVersion)) return;
    autoCheckedRef.current.add(selectedVersion);
    checkModUpdates(selectedVersion);
  }, [selectedVersion, installedVersions]);

  // refresh installed plugin versions when selected version changes
  useEffect(() => {
    refreshInstalledModVersions(selectedVersion);
  }, [selectedVersion, installedVersions]);

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
        // Practice install modal: only show when practice actually installs missing plugins.
        if (event?.payload?.step_name === "Practice Mods") {
          setPracticeTask({
            status: "working",
            ...event.payload,
            error: null,
          });
          const totalFiles = Number(event.payload?.total_files ?? 0);
          if (Number.isFinite(totalFiles) && totalFiles > 0) {
            setPracticePrompt({ open: true });
          }
        }
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
        // refresh installed plugin versions for this game version
        const v = Number(event.payload?.version);
        if (Number.isFinite(v)) refreshInstalledModVersions(v);
      });
      unlistenError = await listen("download://error", (event) => {
        // If practice setup fails, keep the modal open and show the error.
        if (practicePrompt.open && practiceTask?.step_name === "Practice Mods") {
          setPracticeTask((t) => ({
            ...(t ?? {}),
            status: "error",
            version: event.payload?.version ?? t?.version,
            error: event.payload?.message ?? "Unknown error",
          }));
        }
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

  // listen to backend check mods update events
  useEffect(() => {
    let unlistenCheckUpdateProgress = null;
    let unlistenCheckUpdateFinished = null;
    let unlistenCheckUpdateError = null;

    (async () => {
      unlistenCheckUpdateProgress = await listen(
        "updatable://progress",
        (event) => {
        console.log("updatable://progress", event.payload)
          const total = Number(event.payload?.total ?? 0);
          const checked = Number(event.payload?.checked ?? 0);
          const overall_percent =
            total > 0 && Number.isFinite(total) && Number.isFinite(checked)
              ? (checked / total) * 100
              : 0;
          setCheckUpdateTask((t) => ({
            ...t,
            status: "working",
            ...event.payload,
            version: event.payload?.version ?? t.version,
            overall_percent: overall_percent,
            error: null,
          }));
        }
      );
      unlistenCheckUpdateFinished = await listen(
        "updatable://finished",
        (event) => {
          setCheckUpdateTask((t) => ({
            ...t,
            status: "done",
            ...event.payload,
          }));
          // refresh installed versions list after install
          invoke("list_installed_versions")
            .then((v) => setInstalledVersions(Array.isArray(v) ? v : []))
            .catch(() => {});
        }
      );
      unlistenCheckUpdateError = await listen("updatable://error", (event) => {
        setCheckUpdateTask((t) => ({
          ...t,
          status: "error",
          version: event.payload?.version ?? t.version,
          error: event.payload?.message ?? "Unknown error",
        }));
      });
    })();

    return () => {
      if (typeof unlistenCheckUpdateProgress === "function")
        unlistenCheckUpdateProgress();
      if (typeof unlistenCheckUpdateFinished === "function")
        unlistenCheckUpdateFinished();
      if (typeof unlistenCheckUpdateError === "function")
        unlistenCheckUpdateError();
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
      const key = `${String(selectedMod.dev).toLowerCase()}::${String(
        selectedMod.name
      ).toLowerCase()}`;
      setModEnabled(!disabledSet.has(key));

      const files = await invoke("list_config_files_for_mod", {
        dev: selectedMod.dev,
        name: selectedMod.name,
      });
      const list = (Array.isArray(files) ? files : []).filter((p) =>
        String(p).toLowerCase().endsWith(".cfg")
      );
      setConfigFiles(list);
      const next = list[0] ?? "";
      setActiveConfigPath(next);
    })().catch((e) => console.error(e));
  }, [selectedMod, selectedVersion, disabledSet]);

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
      const parsed = await invoke("read_bepinex_cfg", {
        relPath: activeConfigPath,
      });
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

  async function downloadVersion(v, didRetryAfterLogin = false) {
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
      if (
        !didRetryAfterLogin &&
        isAuthError(e) &&
        typeof onRequireLogin === "function"
      ) {
        try {
          await onRequireLogin();
          // After login, retry once automatically.
          return await downloadVersion(v, true);
        } catch {}
      }
      // Backend also emits download://error, but ensure UI reacts if invoke fails early.
      setTask((t) => ({
        ...t,
        status: "error",
        version: v,
        error: e?.message ?? String(e),
      }));
    }
  }

  async function checkModUpdates(v) {
    setCheckUpdateTask((t) => ({
      ...t,
      status: "working",
      version: v,
      overall_percent: 0,
      detail: null,
      updatable_mods: [],
      checked: 0,
      total: 0,
      error: null,
    }));
    try {
      await invoke("check_mod_updates", { version: v });
    } catch (e) {
      setCheckUpdateTask((t) => ({
        ...t,
        status: "error",
        version: v,
        error: e?.message ?? String(e),
      }));
    }
  }

  async function runModUpdate(v) {
    setUpdatePrompt({ open: true });
    setTask((t) => ({
      ...t,
      status: "working",
      version: v,
      overall_percent: 0,
      error: null,
    }));
    try {
      await invoke("apply_mod_updates", { version: v });
      // Avoid re-checking (network heavy). Assume up-to-date after successful apply.
      setCheckUpdateTask((t) => ({
        ...t,
        status: "done",
        version: v,
        overall_percent: 100,
        detail: "Up to date",
        updatable_mods: [],
        checked: 0,
        total: 0,
        error: null,
      }));
      refreshInstalledModVersions(v);
    } catch (e) {
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
              e.name === entryName ? { ...e, value: nextValue } : e
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
      if (
        manifest.chain_config?.find((paths) => paths.includes(activeConfigPath))
      ) {
        let chainPath = manifest.chain_config
          .find((paths) => paths.includes(activeConfigPath))
          .filter((path) => path !== activeConfigPath)[0];
        await invoke("set_bepinex_cfg_entry", {
          args: {
            rel_path: chainPath,
            section: sectionName,
            entry: entryName,
            value: nextValue,
          },
        });
      }
    } catch (e) {
      console.error(e);
      setCfgError(e?.message ?? String(e));
      // re-parse to resync
      try {
        const parsed = await invoke("read_bepinex_cfg", {
          relPath: activeConfigPath,
        });
        setCfgFile(parsed ?? null);
      } catch {}
    } finally {
      setSavingEntry(null);
    }
  }

  function markModBusy(key, busy) {
    setModToggleBusyKeys((prev) => {
      const next = new Set(prev);
      if (busy) next.add(key);
      else next.delete(key);
      return next;
    });
  }

  async function listCfgFilesForMod(mod) {
    if (!mod) return [];
    try {
      const files = await invoke("list_config_files_for_mod", {
        dev: mod.dev,
        name: mod.name,
      });
      return (Array.isArray(files) ? files : [])
        .map((p) => String(p))
        .filter((p) => p.toLowerCase().endsWith(".cfg"));
    } catch {
      return [];
    }
  }

  function chainedPathsForConfigPath(p) {
    const chain = manifest.chain_config?.find((paths) => paths.includes(p));
    return Array.isArray(chain) ? chain : null;
  }

  async function resolveModsForConfigPath(p) {
    const pathLower = String(p ?? "").toLowerCase();
    const mods = Array.isArray(manifest.mods) ? manifest.mods : [];

    // Best-effort: narrow down candidates by substring match, then confirm by asking backend.
    const candidates = mods
      .filter((m) => {
        const d = String(m?.dev ?? "").toLowerCase();
        const n = String(m?.name ?? "").toLowerCase();
        if (!d && !n) return false;
        return (d && pathLower.includes(d)) || (n && pathLower.includes(n));
      })
      .slice(0, 12);

    const confirmed = await Promise.all(
      candidates.map(async (m) => {
        const files = await listCfgFilesForMod(m);
        return files.includes(p) ? m : null;
      })
    );
    return confirmed.filter(Boolean);
  }

  async function toggleModEnabledForMod(mod, nextEnabled, opts) {
    if (!mod) return;
    const propagateChain = opts?.propagateChain ?? true;
    if (!isInstalled(selectedVersion)) {
      openDownloadPrompt(selectedVersion);
      return;
    }

    const baseKey = `${String(mod.dev).toLowerCase()}::${String(
      mod.name
    ).toLowerCase()}`;

    // If the mod has a chained config file, toggle linked mods too.
    let modsToToggle = [mod];
    if (propagateChain) {
      const cfgFiles = await listCfgFilesForMod(mod);
      const linkedPaths = new Set();
      for (const cfgPath of cfgFiles) {
        const chain = chainedPathsForConfigPath(cfgPath);
        if (!chain) continue;
        for (const other of chain) {
          if (other && other !== cfgPath) linkedPaths.add(other);
        }
      }

      if (linkedPaths.size > 0) {
        const linkedMods = await Promise.all(
          Array.from(linkedPaths).map((p) => resolveModsForConfigPath(p))
        );
        for (const list of linkedMods) {
          for (const m of Array.isArray(list) ? list : []) {
            if (!m) continue;
            modsToToggle.push(m);
          }
        }
      }
    }

    // Dedup and skip mods already in desired state (except the primary).
    const seen = new Set();
    modsToToggle = modsToToggle.filter((m) => {
      const k = `${String(m.dev).toLowerCase()}::${String(m.name).toLowerCase()}`;
      if (seen.has(k)) return false;
      seen.add(k);
      if (k !== baseKey) {
        const currentlyEnabled = !disabledSet.has(k);
        if (!!nextEnabled === currentlyEnabled) return false;
      }
      return true;
    });

    const toggledKeys = modsToToggle.map(
      (m) => `${String(m.dev).toLowerCase()}::${String(m.name).toLowerCase()}`
    );
    for (const k of toggledKeys) markModBusy(k, true);

    const selectedKey =
      selectedMod &&
      `${String(selectedMod.dev).toLowerCase()}::${String(
        selectedMod.name
      ).toLowerCase()}`;
    const affectsSelected = selectedKey && toggledKeys.includes(selectedKey);
    if (affectsSelected) setModToggleBusy(true);
    try {
      await Promise.all(
        modsToToggle.map((m) =>
          invoke("set_mod_enabled", {
            version: selectedVersion,
            dev: m.dev,
            name: m.name,
            enabled: !!nextEnabled,
          })
        )
      );
      // refresh disabled list (source of truth)
      const dm = await invoke("get_disabled_mods");
      setDisabledMods(Array.isArray(dm) ? dm : []);
      if (affectsSelected) setModEnabled(!!nextEnabled);
    } catch (e) {
      console.error(e);
      setCfgError(e?.message ?? String(e));
    } finally {
      for (const k of toggledKeys) markModBusy(k, false);
      if (affectsSelected) setModToggleBusy(false);
    }
  }

  async function toggleModEnabled(nextEnabled) {
    if (!selectedMod) return;
    return toggleModEnabledForMod(selectedMod, nextEnabled);
  }

  const versionOptions = useMemo(() => {
    const set = new Set(installedVersions);
    set.add(selectedVersion);
    // show versions provided by remote manifest (version -> download_manifest)
    const remoteV =
      manifest?.manifests && typeof manifest.manifests === "object"
        ? Object.keys(manifest.manifests)
            .map((k) => Number(k))
            .filter((n) => Number.isFinite(n))
        : [];
    remoteV.forEach((v) => set.add(v));
    return Array.from(set).sort((a, b) => b - a);
  }, [installedVersions, selectedVersion, manifest]);

  const selectedInstalled = isInstalled(selectedVersion);
  const promptVersion = downloadPrompt.version;
  const promptIsWorking =
    downloadPrompt.open &&
    task.status === "working" &&
    task.version === promptVersion;
  const promptIsDone =
    downloadPrompt.open &&
    task.status === "done" &&
    task.version === promptVersion;
  const promptIsError =
    downloadPrompt.open &&
    task.status === "error" &&
    task.version === promptVersion;

  const [downloadCancelBusy, setDownloadCancelBusy] = useState(false);

  function resetTaskForVersion(v) {
    if (typeof v !== "number") return;
    setTask((t) => {
      if (t.version !== v) return t;
      return {
        status: "idle",
        version: null,
        step_name: null,
        steps_total: null,
        step: null,
        overall_percent: null,
        detail: null,
        downloaded_bytes: null,
        total_bytes: null,
        error: null,
      };
    });
  }

  function closeDownloadPrompt() {
    const v = downloadPrompt.version;
    setDownloadPrompt({ open: false, version: null });
    setDownloadCancelBusy(false);
    resetTaskForVersion(v);
  }

  const updateIsWorking = updatePrompt.open && task.status === "working";
  const updateIsDone = updatePrompt.open && task.status === "done";
  const updateIsError = updatePrompt.open && task.status === "error";

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
      setGameStatus({
        running: true,
        pid: typeof pid === "number" ? pid : null,
      });
      // backend may force-disable practice mods on normal run
      invoke("get_disabled_mods")
        .then((dm) => setDisabledMods(Array.isArray(dm) ? dm : []))
        .catch(() => {});
    } catch (e) {
      console.error(e);
      setTask((t) => ({
        ...t,
        status: "error",
        version: selectedVersion,
        error: e?.message ?? String(e),
      }));
    }
  }

  async function startPracticeRun() {
    if (gameStatus.running) return;
    if (!isInstalled(selectedVersion)) {
      openDownloadPrompt(selectedVersion);
      return;
    }
    try {
      // Reset practice modal state; it will open only if backend reports installs needed.
      setPracticePrompt({ open: false });
      setPracticeTask(null);
      const pid = await invoke("launch_game_practice", { version: selectedVersion });
      setPracticePrompt({ open: false });
      setGameStatus({
        running: true,
        pid: typeof pid === "number" ? pid : null,
      });
      // backend may enable/disable practice mods for this version
      invoke("get_disabled_mods")
        .then((dm) => setDisabledMods(Array.isArray(dm) ? dm : []))
        .catch(() => {});
    } catch (e) {
      console.error(e);
      // If there's an error without progress, show it via the practice modal too.
      setPracticePrompt({ open: true });
      setPracticeTask((t) => ({
        ...(t ?? {}),
        status: "error",
        version: selectedVersion,
        step_name: t?.step_name ?? "Practice Mods",
        detail: t?.detail ?? "Failed to prepare practice mods",
        error: e?.message ?? String(e),
      }));
      setTask((t) => ({
        ...t,
        status: "error",
        version: selectedVersion,
        error: e?.message ?? String(e),
      }));
    }
  }

  async function startSelectedRun() {
    if (gameStatus.running) return startRun(); // stop
    if (runMode === "practice") return startPracticeRun();
    return startRun();
  }

  const runModeLabel = runMode === "practice" ? "Practice" : "Normal";

  return (
    <div className="h-full text-white">
      <div className="mx-auto flex h-full max-w-[1600px] flex-col gap-4 p-4">
        {/* Top bar */}
        <div className="flex items-center gap-3">
          {gameStatus.running ? (
            <Button variant="secondary" className="h-11 px-5" onClick={startRun} title="Stop">
              <Play className="h-4 w-4" />
              {runModeLabel} Stop
            </Button>
          ) : (
            <Select value={runMode} onValueChange={(v) => setRunMode(v)}>
              <SelectTrigger
                showIcon={false}
                className="h-11 w-fit font-semibold overflow-hidden rounded-xl border-white/10 bg-white px-0 text-black hover:bg-white/90 focus:ring-white/15"
                title={
                  runMode === "practice"
                    ? "Practice run: installs/enables practice mods for this run"
                    : "Normal run: practice mods are disabled"
                }
              >
                <div className="flex h-full w-full items-stretch">
                  <div
                    className="flex h-full select-none items-center gap-2 px-5"
                    onPointerDown={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      startSelectedRun();
                    }}
                    onClick={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                    }}
                  >
                    <Play className="h-4 w-4" />
                    {runMode === "practice" ? "Practice Run" : "Start Run"}
                  </div>
                  <div className="flex w-10 items-center justify-center border-l border-black/10">
                    <ChevronDown className="h-4 w-4 text-black/70" />
                    <span className="sr-only">Select run mode</span>
                  </div>
                </div>
              </SelectTrigger>
              <SelectContent className="min-w-48" align="start">
                <SelectItem value="normal">Normal Run</SelectItem>
                <SelectItem value="practice">Practice Run</SelectItem>
              </SelectContent>
            </Select>
          )}

          <div className="w-fit">
            <Select
              value={String(selectedVersion)}
              onValueChange={(v) => {
                const nextV = Number(v);
                if (isInstalled(nextV)) {
                  setSelectedVersion(nextV);
                  invoke("apply_disabled_mods", { version: nextV }).catch(
                    () => {}
                  );
                } else {
                  openDownloadPrompt(nextV);
                }
              }}
            >
              <SelectTrigger className="h-11 px-3">
                <div className="mr-2 flex items-center gap-2">
                  <div className="text-sm font-semibold">
                    v{selectedVersion}
                  </div>
                  {selectedInstalled ? (
                    <CheckCircle2 className="h-4 w-4 text-emerald-400" />
                  ) : (
                    <Download className="h-4 w-4 text-amber-300" />
                  )}
                </div>
              </SelectTrigger>
              <SelectContent>
                {versionOptions.map((v) => (
                  <SelectItem
                    key={v}
                    value={String(v)}
                    marker={
                      isInstalled(v) ? null : (
                        <Download className="h-4 w-4 text-amber-300" />
                      )
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

          {checkUpdateTask.updatable_mods.length > 0 && <Button
            variant="secondary"
            className="h-11"
            onClick={() => {
              const sameVersion = checkUpdateTask.version === selectedVersion;
              const alreadyChecked = sameVersion && checkUpdateTask.status === "done";
              const isChecking = sameVersion && checkUpdateTask.status === "working";

              // Don't re-check if we already have results for this version.
              // If it's currently checking, just open the modal to show progress.
              if (!alreadyChecked && !isChecking) checkModUpdates(selectedVersion);
              setCheckUpdatePrompt({ open: true, mods: filteredMods });
            }}
            title="Check mod updates"
          >
            <span className="relative inline-flex">
              <Download className="h-4 w-4" />
              {checkUpdateTask.status === "done" &&
              checkUpdateTask.version === selectedVersion &&
              (checkUpdateTask.updatable_mods?.length ?? 0) > 0 ? (
                <span className="absolute -right-2 -top-2 rounded-full bg-amber-400 px-1.5 py-0.5 text-[10px] font-bold text-black">
                  {checkUpdateTask.updatable_mods.length}
                </span>
              ) : null}
            </span>
          </Button>}
          {loginState?.username != null && <div className="ml-2 flex items-center gap-2">
            {loginState?.username ? (
              <div className="hidden items-center gap-2 md:flex">
                <div className="text-sm text-white/55">
                  logged In
                </div>
              </div>
            ) : null}
            <Button
              variant="secondary"
              className="h-11"
              onClick={() => onLogout?.()}
              title="Logout"
            >
              <LogOut className="h-4 w-4" />
            </Button>
          </div>}
        </div>

        {/* Main grid */}
        <div className="grid min-h-0 flex-1 grid-cols-[repeat(auto-fit,minmax(0,1fr))] gap-4">
          {/* Mod list */}
          <div className="min-h-0 rounded-2xl border border-white/10 bg-white/5 p-3">
            <div className="mb-3 flex items-center justify-between px-1">
              <div className="text-sm font-semibold text-white/80">
                Mods{" "}
              </div>
              <div className="text-xs text-white/40">
                {filteredMods.length} items
              </div>
            </div>

            <div className="h-[calc(100%-2.25rem)] overflow-auto pr-1">
              <div className="flex flex-col gap-2">
                {filteredMods.map((m) => {
                  const selected =
                    selectedMod && modKey(selectedMod) === modKey(m);
                  const initials = `${m.dev?.[0] ?? "M"}${
                    m.name?.[0] ?? "M"
                  }`.toUpperCase();
                  const keyLower = `${String(m.dev).toLowerCase()}::${String(
                    m.name
                  ).toLowerCase()}`;
                  const enabled = !disabledSet.has(keyLower);
                  const installedVer = installedModVersions[keyLower];
                  const busy = modToggleBusyKeys.has(keyLower);
                  return (
                    ((!isInstalled(selectedVersion)) || installedVer) && <div
                      key={modKey(m)}
                      className={cn(
                        "group flex w-full items-start gap-3 rounded-2xl border px-3 py-3 text-left transition",
                        selected
                          ? "border-white/20 bg-white/10"
                          : "border-white/10 bg-black/10 hover:bg-white/10",
                          installedVer || "opacity-40"
                      )}
                      onClick={() => setSelectedMod(m)}
                      role="button"
                      tabIndex={0}
                      onKeyDown={(e) => {
                        if (e.key === "Enter" || e.key === " ") {
                          e.preventDefault();
                          setSelectedMod(m);
                        }
                      }}
                    >
                      <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl bg-white/10 text-sm font-bold text-white/80">
                        {initials}
                      </div>
                      <div className="min-w-0 flex-1">
                        <div className="flex items-baseline gap-2">
                          <div className="truncate text-base font-semibold">
                            {m.name}
                          </div>
                          <div className="truncate text-sm text-white/40">
                            {m.dev}
                          </div>
                        </div>
                        <div className="mt-1 line-clamp-1 text-sm text-white/50">
                          Click to edit config
                        </div>
                      </div>
                      <div className="self-stretch flex shrink-0 items-center">
                        {/* <div
                          className={cn(
                            "rounded-full border border-white/10 bg-white/5 px-2 py-0.5 text-[11px]",
                            installedVer ? "text-white/60" : "text-white/35"
                          )}
                          title={
                            installedVer
                              ? `Installed: v${installedVer}`
                              : "Not installed"
                          }
                        >
                          {installedVer ? `v${installedVer}` : "not installed"}
                        </div> */}
                        <div
                          onClick={(e) => e.stopPropagation()}
                          onKeyDown={(e) => e.stopPropagation()}
                          className="inline-flex"
                        >
                          <Switch
                            checked={enabled}
                            disabled={busy}
                            onCheckedChange={(v) =>
                              toggleModEnabledForMod(m, !!v)
                            }
                          />
                        </div>
                      </div>
                      {/* <Settings2 className="mt-1 h-4 w-4 shrink-0 text-white/30 opacity-0 transition group-hover:opacity-100" /> */}
                    </div>
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
                    <div className="truncate text-lg font-semibold">
                      {selectedMod.name}
                    </div>
                    <div className="truncate text-sm text-white/40">
                      {selectedMod.dev}
                    </div>
                    <div className="mt-1 text-xs text-white/45">
                      {(() => {
                        const k = `${String(selectedMod.dev).toLowerCase()}::${String(
                          selectedMod.name
                        ).toLowerCase()}`;
                        const v = installedModVersions[k];
                        return v ? `Installed: v${v}` : "Not installed";
                      })()}
                    </div>
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

                {/* <div className="flex items-center justify-between rounded-2xl border border-white/10 bg-black/10 px-3 py-2">
                  <div className="text-sm font-semibold text-white/80">
                    Enabled
                  </div>
                  <div className="flex items-center gap-2">
                    <div className="text-xs text-white/50">
                      {modToggleBusy
                        ? "Applying..."
                        : modEnabled
                        ? "On"
                        : "Off"}
                    </div>
                    <Switch
                      checked={modEnabled}
                      disabled={modToggleBusy}
                      onCheckedChange={(v) => toggleModEnabled(!!v)}
                    />
                  </div>
                </div> */}

                <div className="flex items-center gap-2">
                  <div className="text-xs font-semibold text-white/50">
                    Section
                  </div>
                  <div className="flex-1">
                    <Select
                      value={activeSection}
                      onValueChange={(v) => setActiveSection(v)}
                      disabled={
                        !cfgFile || (cfgFile.sections ?? []).length === 0
                      }
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
                    File:{" "}
                    <span className="text-white/50">{activeConfigPath}</span>
                    {manifest.chain_config?.find((paths) =>
                      paths.includes(activeConfigPath)
                    ) && (
                      <span className="text-white/50">
                        {" "}
                        (also affects{" "}
                        {
                          manifest.chain_config
                            .find((paths) => paths.includes(activeConfigPath))
                            .filter((path) => path !== activeConfigPath)[0]
                        }
                        )
                      </span>
                    )}
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
                        const s =
                          (cfgFile.sections ?? []).find(
                            (x) => x.name === activeSection
                          ) ?? cfgFile.sections?.[0];
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
                                      <div className="truncate text-sm font-semibold">
                                        {e.name}
                                      </div>
                                      {e.description && (
                                        <div className="mt-1 whitespace-pre-wrap text-xs text-white/50">
                                          {e.description}
                                        </div>
                                      )}
                                    </div>
                                    <div className="shrink-0 text-[10px] text-white/40">
                                      {savingEntry === id
                                        ? "Saving..."
                                        : v?.type ?? ""}
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
                                        <span className="text-white/80">
                                          Enabled
                                        </span>
                                      </label>
                                    ) : v?.type === "Int" ? (
                                      v.data?.range ? (
                                        <div className="flex items-center gap-3">
                                          <div className="w-20 shrink-0 text-xs text-white/50">
                                            {v.data.range.start}-
                                            {v.data.range.end}
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
                                            {v.data.range.start}-
                                            {v.data.range.end}
                                          </div>
                                          <Slider
                                            value={[v.data.value ?? 0]}
                                            min={v.data.range.start}
                                            max={v.data.range.end}
                                            step={
                                              (v.data.range.end -
                                                v.data.range.start) /
                                                200 || 0.01
                                            }
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
                                          <input
                                            className="w-13 shrink-0 text-sm text-white/80 tabular-nums"
                                            type="text"
                                            value={v.data?.value ?? 0}
                                            onChange={(ev) =>
                                              setCfgEntry(s.name, e.name, {
                                                type: "Float",
                                                data: {
                                                  value: Number(
                                                    ev.target.value
                                                  ),
                                                  range: v.data?.range ?? null,
                                                },
                                              })
                                            }
                                          />
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
                                          {(v.data?.options ?? []).map(
                                            (opt, idx) => (
                                              <SelectItem
                                                key={opt}
                                                value={String(idx)}
                                              >
                                                {opt}
                                              </SelectItem>
                                            )
                                          )}
                                        </SelectContent>
                                      </Select>
                                    ) : v?.type === "Flags" ? (
                                      <div className="flex flex-col gap-2">
                                        {(v.data?.options ?? []).map(
                                          (opt, idx) => {
                                            const checked = (
                                              v.data?.indicies ?? []
                                            ).includes(idx);
                                            return (
                                              <label
                                                key={opt}
                                                className="flex cursor-pointer items-center gap-2 text-sm text-white/80"
                                              >
                                                <Checkbox
                                                  checked={checked}
                                                  onCheckedChange={(
                                                    nextChecked
                                                  ) => {
                                                    const set = new Set(
                                                      v.data?.indicies ?? []
                                                    );
                                                    if (nextChecked)
                                                      set.add(idx);
                                                    else set.delete(idx);
                                                    setCfgEntry(
                                                      s.name,
                                                      e.name,
                                                      {
                                                        type: "Flags",
                                                        data: {
                                                          indicies: Array.from(
                                                            set
                                                          ).sort(
                                                            (a, b) => a - b
                                                          ),
                                                          options:
                                                            v.data?.options ??
                                                            [],
                                                        },
                                                      }
                                                    );
                                                  }}
                                                />
                                                <span>{opt}</span>
                                              </label>
                                            );
                                          }
                                        )}
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
              if (!promptIsWorking) closeDownloadPrompt();
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
                  This version is not downloaded yet. Do you want to download it
                  now?
                </div>
              </div>
            </div>

            {/* Progress (from Rust emit) */}
            {(promptIsWorking || promptIsDone || promptIsError) && (
              <div className="mt-4 rounded-2xl border border-white/10 bg-white/5 p-3">
                <div className="flex items-center justify-between gap-3">
                  <div className="min-w-0">
                    <div className="truncate text-sm font-semibold">
                      {statusText}
                    </div>
                    <div className="truncate text-xs text-white/50">
                      {task.detail ||
                        (bytesText ? `Downloaded: ${bytesText}` : "")}
                    </div>
                    {task.error && (
                      <div className="mt-1 text-xs text-red-300">
                        {task.error}
                      </div>
                    )}
                  </div>
                  <div className="shrink-0 text-sm text-white/70">
                    {progressText}
                  </div>
                </div>
                <div className="mt-2 h-2 w-full overflow-hidden rounded-full bg-white/10">
                  <div
                    className={cn(
                      "h-full rounded-full transition-[width]",
                      task.status === "error" ? "bg-red-400" : "bg-emerald-400"
                    )}
                    style={{
                      width: `${Math.max(
                        0,
                        Math.min(100, task.overall_percent ?? 0)
                      )}%`,
                    }}
                  />
                </div>
              </div>
            )}

            <div className="mt-5 flex items-center justify-end gap-2">
              {promptIsWorking ? (
                <>
                  <Button
                    variant="secondary"
                    className="h-10"
                    disabled={downloadCancelBusy}
                    onClick={async () => {
                      if (typeof promptVersion !== "number") return;
                      setDownloadCancelBusy(true);
                      try {
                        await invoke("cancel_download", { version: promptVersion });
                      } catch (e) {
                        // Best-effort; backend will emit an error if cancel fails.
                        console.error(e);
                      } finally {
                        setDownloadCancelBusy(false);
                      }
                    }}
                  >
                    Cancel
                  </Button>
                  <Button
                    variant="default"
                    disabled
                    className="h-10 min-w-[120px]"
                  >
                    <Download className="h-4 w-4" />
                    Downloading...
                  </Button>
                </>
              ) : promptIsDone ? (
                <Button
                  variant="default"
                  className="h-10 min-w-[120px]"
                  onClick={closeDownloadPrompt}
                >
                  Close
                </Button>
              ) : promptIsError ? (
                <>
                  <Button variant="secondary" className="h-10" onClick={closeDownloadPrompt}>
                    Close
                  </Button>
                  <Button
                    variant="default"
                    className="h-10 min-w-[120px]"
                    onClick={() => {
                      if (typeof promptVersion !== "number") return;
                      setSelectedVersion(promptVersion);
                      downloadVersion(promptVersion);
                    }}
                    disabled={typeof promptVersion !== "number"}
                  >
                    Retry
                  </Button>
                </>
              ) : (
                <>
                  <Button
                    variant="secondary"
                    onClick={closeDownloadPrompt}
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

      {/* Check update confirm modal */}
      {checkUpdatePrompt.open && (
        <div className="fixed inset-0 z-50 flex items-center justify-center">
          <button
            className="absolute inset-0 bg-black/60"
            onClick={() => {
              if (checkUpdateTask.status !== "working")
                setCheckUpdatePrompt({ open: false, mods: [] });
            }}
            aria-label="Close"
          />

          <div className="relative w-[min(520px,calc(100vw-2rem))] rounded-2xl border border-white/10 bg-[#0f1116] p-5 shadow-2xl shadow-black/50">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <div className="text-lg font-semibold">
                  {checkUpdateTask.status === "done"
                    ? `${checkUpdateTask.updatable_mods.length} mods can be updated`
                    : "Checking for updates..."}
                </div>
              </div>
            </div>

            {(checkUpdateTask.status === "working" ||
              checkUpdateTask.status === "error") && (
              <div className="mt-4 rounded-2xl">
                <div className="h-2 w-full overflow-hidden rounded-full bg-gray-500">
                  <div
                    className={cn(
                      "h-full rounded-full transition-[width]",
                      checkUpdateTask.status === "error"
                        ? "bg-red-400"
                        : "bg-emerald-400"
                    )}
                    style={{
                      width: `${Math.max(
                        0,
                        Math.min(100, checkUpdateTask.overall_percent ?? 0)
                      )}%`,
                    }}
                  />
                </div>

                <div className="mt-2 flex items-center justify-between gap-3 text-sm text-white/50">
                  {checkUpdateTask.detail}
                </div>
              </div>
            )}
            {checkUpdateTask.status === "done" && (
              <div className="mt-4 text-sm text-white/50">
                {checkUpdateTask.updatable_mods.map((mod, index) => (
                  <div key={index}>{mod}</div>
                ))}
              </div>
            )}

            <div className="mt-5 flex items-center justify-end gap-2">
              {checkUpdateTask.status === "working" ? (
                <Button
                  variant="default"
                  disabled
                  className="h-10 min-w-[120px]"
                >
                  <Download className="h-4 w-4" />
                  Checking...
                </Button>
              ) : (
                <>
                  <Button
                    variant="secondary"
                    className="h-10 min-w-[120px]"
                    onClick={() =>
                      setCheckUpdatePrompt({ open: false, mods: [] })
                    }
                  >
                    Close
                  </Button>
                  {checkUpdateTask.status === "done" && (
                    <Button
                      variant="default"
                      className="h-10 min-w-[120px]"
                      disabled={(checkUpdateTask.updatable_mods?.length ?? 0) === 0}
                      onClick={() => {
                        setCheckUpdatePrompt({ open: false, mods: [] });
                        runModUpdate(checkUpdateTask.version ?? selectedVersion);
                      }}
                    >
                      Update
                    </Button>
                  )}
                </>
              )}
            </div>
          </div>
        </div>
      )}

      {/* Manifest update modal (uses download://progress events) */}
      {updatePrompt.open && (
        <div className="fixed inset-0 z-50 flex items-center justify-center">
          <button
            className="absolute inset-0 bg-black/60"
            onClick={() => {
              if (!updateIsWorking) setUpdatePrompt({ open: false });
            }}
            aria-label="Close"
          />

          <div className="relative w-[min(520px,calc(100vw-2rem))] rounded-2xl border border-white/10 bg-[#0f1116] p-5 shadow-2xl shadow-black/50">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <div className="text-lg font-semibold">
                  {updateIsDone
                    ? "Update complete"
                    : updateIsError
                    ? "Update failed"
                    : "Updating..."}
                </div>
                <div className="mt-1 text-sm text-white/55">
                  Remote manifest 기준으로 config/mods를 최신 설치 버전에
                  반영합니다.
                </div>
              </div>
            </div>

            {(updateIsWorking || updateIsDone || updateIsError) && (
              <div className="mt-4 rounded-2xl border border-white/10 bg-white/5 p-3">
                <div className="flex items-center justify-between gap-3">
                  <div className="min-w-0">
                    <div className="truncate text-sm font-semibold">
                      {statusText}
                    </div>
                    <div className="truncate text-xs text-white/50">
                      {task.detail || ""}
                    </div>
                    {task.error && (
                      <div className="mt-1 text-xs text-red-300">
                        {task.error}
                      </div>
                    )}
                  </div>
                  <div className="shrink-0 text-sm text-white/70">
                    {progressText}
                  </div>
                </div>
                <div className="mt-2 h-2 w-full overflow-hidden rounded-full bg-white/10">
                  <div
                    className={cn(
                      "h-full rounded-full transition-[width]",
                      updateIsError ? "bg-red-400" : "bg-emerald-400"
                    )}
                    style={{
                      width: `${Math.max(
                        0,
                        Math.min(100, task.overall_percent ?? 0)
                      )}%`,
                    }}
                  />
                </div>
              </div>
            )}

            <div className="mt-5 flex items-center justify-end gap-2">
              {updateIsWorking ? (
                <Button
                  variant="default"
                  disabled
                  className="h-10 min-w-[120px]"
                >
                  Updating...
                </Button>
              ) : (
                <Button
                  variant="secondary"
                  className="h-10 min-w-[120px]"
                  onClick={() => setUpdatePrompt({ open: false })}
                >
                  Close
                </Button>
              )}
            </div>
          </div>
        </div>
      )}

      {/* Practice install modal (only shows when installing plugins) */}
      {practicePrompt.open && (
        <div className="fixed inset-0 z-50 flex items-center justify-center">
          <button
            className="absolute inset-0 bg-black/60"
            onClick={() => {
              const st = practiceTask?.status ?? "working";
              if (st !== "working") setPracticePrompt({ open: false });
            }}
            aria-label="Close"
          />

          <div className="relative w-[min(520px,calc(100vw-2rem))] rounded-2xl border border-white/10 bg-[#0f1116] p-5 shadow-2xl shadow-black/50">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <div className="text-lg font-semibold">
                  {practiceTask?.status === "error"
                    ? "Practice setup failed"
                    : "Installing practice mods..."}
                </div>
                <div className="mt-1 text-sm text-white/55">
                  Required practice plugins are being installed for this run.
                </div>
              </div>
            </div>

            <div className="mt-4 rounded-2xl border border-white/10 bg-white/5 p-3">
              <div className="flex items-center justify-between gap-3">
                <div className="min-w-0">
                  <div className="truncate text-sm font-semibold">
                    {practiceTask?.step_name ?? "Practice Mods"}
                  </div>
                  <div className="truncate text-xs text-white/50">
                    {practiceTask?.detail ?? ""}
                  </div>
                  {practiceTask?.error && (
                    <div className="mt-1 text-xs text-red-300">
                      {practiceTask.error}
                    </div>
                  )}
                </div>
                <div className="shrink-0 text-sm text-white/70">
                  {Number.isFinite(Number(practiceTask?.overall_percent))
                    ? `${Math.round(Number(practiceTask?.overall_percent))}%`
                    : ""}
                </div>
              </div>
              <div className="mt-2 h-2 w-full overflow-hidden rounded-full bg-white/10">
                <div
                  className={cn(
                    "h-full rounded-full transition-[width]",
                    practiceTask?.status === "error" ? "bg-red-400" : "bg-emerald-400"
                  )}
                  style={{
                    width: `${Math.max(
                      0,
                      Math.min(100, Number(practiceTask?.overall_percent ?? 0))
                    )}%`,
                  }}
                />
              </div>
            </div>

            <div className="mt-5 flex items-center justify-end gap-2">
              <Button
                variant="secondary"
                className="h-10 min-w-[120px]"
                disabled={(practiceTask?.status ?? "working") === "working"}
                onClick={() => setPracticePrompt({ open: false })}
              >
                Close
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
