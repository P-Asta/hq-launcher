import { useEffect, useMemo, useRef, useState, useCallback } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import {
  Check,
  CheckCircle2,
  ChevronDown,
  Download,
  LogOut,
  LoaderCircle,
  Play,
  Search,
  Settings2,
  Trash2,
  X,
} from "lucide-react";
import { Button } from "../components/ui/button";
import { Dialog, DialogContent } from "../components/ui/dialog";
import { Input } from "../components/ui/input";
import { Checkbox } from "../components/ui/checkbox";
import { Switch } from "../components/ui/switch";
import { Slider } from "../components/ui/slider";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectSeparator,
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

function formatTransferProgress(task) {
  const downloaded = Number(task?.downloaded_bytes);
  if (!Number.isFinite(downloaded)) return "";
  const total = Number(task?.total_bytes);
  if (Number.isFinite(total) && total > 0) {
    return `Download ${fmtBytes(downloaded)} / ${fmtBytes(total)}`;
  }
  return downloaded > 0 ? `Download ${fmtBytes(downloaded)}` : "";
}

function formatExtractProgress(task) {
  const done = Number(task?.extracted_files);
  const total = Number(task?.total_files);
  if (Number.isFinite(done) && Number.isFinite(total) && total > 0) {
    return `Extract ${done.toLocaleString()} / ${total.toLocaleString()} files`;
  }
  return "";
}

function makeDeleteVersionPromptState(overrides = {}) {
  return {
    open: false,
    version: null,
    error: "",
    status: "idle",
    overall_percent: 0,
    detail: "",
    deleted_files: 0,
    total_files: 0,
    ...overrides,
  };
}

const MOD_PANEL_WIDTH_STORAGE_KEY = "launcherModPanelWidthPercent";
const DEFAULT_MOD_PANEL_WIDTH = 40;
const MIN_MOD_PANEL_WIDTH = 260;
const MIN_CONFIG_PANEL_WIDTH = 320;

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}

function modKey(mod) {
  return `${mod.dev}::${mod.name}`;
}

function modKeyLower(mod) {
  return `${String(mod?.dev ?? "").toLowerCase()}::${String(
    mod?.name ?? ""
  ).toLowerCase()}`;
}

function isPracticeRunMode(mode) {
  return String(mode ?? "").toLowerCase().includes("practice");
}

function isSmhqRunMode(mode) {
  return String(mode ?? "").toLowerCase().includes("smhq");
}

function isUiHiddenMod(mod) {
  return Array.isArray(mod?.tags)
    ? mod.tags.some((tag) => String(tag).toLowerCase() === "ui_hidden")
    : false;
}

function sleep(ms) {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

function toOptionalNumber(value) {
  if (value == null || value === "") return null;
  const n = Number(value);
  return Number.isFinite(n) ? n : null;
}

function isModCompatibleWithVersion(mod, version) {
  const v = Number(version);
  if (!Number.isFinite(v)) return true;
  const lowCap = toOptionalNumber(mod?.low_cap);
  const highCap = toOptionalNumber(mod?.high_cap);
  if (lowCap != null && v < lowCap) return false;
  if (highCap != null && v > highCap) return false;
  return true;
}

function getRunModePresetTags(mode) {
  if (mode === "brutal" || mode === "brutal_practice") return ["Brutal"];
  if (mode === "c_moons" || mode === "c_moons_practice" || mode === "c_moons_smhq")
    return ["C.Moons"];
  if (mode === "wesley" || mode === "wesley_practice" || mode === "wesley_smhq")
    return ["Wesley"];
  return [];
}

function getManifestPresetConstraint(manifest, tag) {
  const entries = Object.entries(manifest?.preset_tag_constraints ?? {});
  const match = entries.find(([key]) => String(key).toLowerCase() === String(tag).toLowerCase());
  return match?.[1] ?? null;
}

function getPresetVersionRange(manifest, mode) {
  const tags = getRunModePresetTags(mode);
  let low = null;
  let high = null;

  for (const tag of tags) {
    const rule = getManifestPresetConstraint(manifest, tag);
    if (!rule) continue;
    const lowCap = toOptionalNumber(rule?.low_cap);
    const highCap = toOptionalNumber(rule?.high_cap);
    if (lowCap != null) low = low == null ? lowCap : Math.max(low, lowCap);
    if (highCap != null) high = high == null ? highCap : Math.min(high, highCap);
  }

  if (low == null && high == null) return null;
  return { low, high };
}

function isVersionWithinRange(version, range) {
  const v = Number(version);
  if (!Number.isFinite(v) || !range) return true;
  if (range.low != null && v < range.low) return false;
  if (range.high != null && v > range.high) return false;
  return true;
}

function clampVersionToRange(version, range) {
  const v = Number(version);
  if (!Number.isFinite(v) || !range) return v;
  if (range.low != null && v < range.low) return range.low;
  if (range.high != null && v > range.high) return range.high;
  return v;
}

const PRACTICE_LOCKED_MOD_KEYS = new Set([
  "hqhqteam::vlog",
]);

const SMHQ_FORCED_MOD_KEYS = new Set([
  "slushyrh::freeeeeemoooooons",
]);

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

const RUN_MODE_VALUES = [
  "hq",
  "smhq",
  "c_moons",
  "c_moons_smhq",
  "c_moons_practice",
  "practice",
  "brutal",
  "brutal_practice",
  "wesley",
  "wesley_smhq",
  "wesley_practice",
];

const DISCORD_DOWNLOAD_URL = "https://asta.rs/hq-launcher/";

function getInitialRunMode() {
  const savedRunMode = localStorage.getItem("selectedRunMode");
  return RUN_MODE_VALUES.includes(savedRunMode) ? savedRunMode : "hq";
}

function getLaunchRequestForRunMode(mode, version) {
  if (mode === "practice") {
    return {
      command: "launch_game_practice",
      args: { version },
    };
  }
  if (mode === "brutal") {
    return {
      command: "launch_game_preset",
      args: { version, preset: "brutal", practice: false },
    };
  }
  if (mode === "brutal_practice") {
    return {
      command: "launch_game_preset",
      args: { version, preset: "brutal", practice: true },
    };
  }
  if (mode === "wesley") {
    return {
      command: "launch_game_preset",
      args: { version, preset: "wesley", practice: false },
    };
  }
  if (mode === "wesley_practice") {
    return {
      command: "launch_game_preset",
      args: { version, preset: "wesley", practice: true },
    };
  }
  if (mode === "wesley_smhq") {
    return {
      command: "launch_game_preset",
      args: { version, preset: "wesley_smhq", practice: false },
    };
  }
  if (mode === "smhq") {
    return {
      command: "launch_game_preset",
      args: { version, preset: "smhq", practice: false },
    };
  }
  if (mode === "c_moons") {
    return {
      command: "launch_game_preset",
      args: { version, preset: "c_moons", practice: false },
    };
  }
  if (mode === "c_moons_practice") {
    return {
      command: "launch_game_preset",
      args: { version, preset: "c_moons", practice: true },
    };
  }
  if (mode === "c_moons_smhq") {
    return {
      command: "launch_game_preset",
      args: { version, preset: "c_moons_smhq", practice: false },
    };
  }
  return {
    command: "launch_game",
    args: { version },
  };
}

function isPresetSummaryMod(mod) {
  return mod?.isPresetSummary === true;
}

function listEntryKey(entry) {
  return entry?.summary_id ?? modKey(entry);
}

function matchesVersionCaps(version, lowCap, highCap) {
  const v = Number(version);
  if (!Number.isFinite(v)) return true;
  const low = toOptionalNumber(lowCap);
  const high = toOptionalNumber(highCap);
  if (low != null && v < low) return false;
  if (high != null && v > high) return false;
  return true;
}

function findTagConstraint(mod, activeTag) {
  const constraints = mod?.tag_constraints;
  if (!constraints || typeof constraints !== "object") return null;
  for (const [tag, rule] of Object.entries(constraints)) {
    if (String(tag).toLowerCase() === String(activeTag).toLowerCase()) {
      return rule ?? null;
    }
  }
  return null;
}

function isModCompatibleWithTags(mod, version, activeTags) {
  const modTags = Array.isArray(mod?.tags) ? mod.tags : [];
  if (modTags.length === 0) return false;

  for (const activeTag of Array.isArray(activeTags) ? activeTags : []) {
    const matchesTag = modTags.some(
      (tag) => String(tag).toLowerCase() === String(activeTag).toLowerCase()
    );
    if (!matchesTag) continue;

    const constraint = findTagConstraint(mod, activeTag);
    const lowCap = constraint?.low_cap ?? mod?.low_cap ?? null;
    const highCap = constraint?.high_cap ?? mod?.high_cap ?? null;
    if (matchesVersionCaps(version, lowCap, highCap)) {
      return true;
    }
  }

  return false;
}

function getPresetSummarySpec(mode) {
  if (mode === "brutal" || mode === "brutal_practice") {
    return {
      summary_id: "preset::brutal",
      name: "Brutal Mods",
      subtitle: "",
      activeTags: ["Brutal"],
      iconKey: "drinkablewater::brutal_company_minus",
    };
  }

  if (mode === "wesley" || mode === "wesley_practice") {
    return {
      summary_id: "preset::wesley",
      name: "Wesley's Mods",
      subtitle: "",
      activeTags: ["Wesley"],
      iconKey: "magic_wesley::wesleys_moons",
    };
  }

  if (mode === "wesley_smhq") {
    return {
      summary_id: "preset::wesley_smhq",
      name: "Wesley's Mods",
      subtitle: "",
      activeTags: ["Wesley", "SMHQ"],
      iconKey: "magic_wesley::wesleys_moons",
    };
  }

  if (mode === "c_moons" || mode === "c_moons_practice") {
    return {
      summary_id: "preset::c_moons",
      name: "C.Moons Mods",
      subtitle: "",
      activeTags: ["C.Moons"],
      iconKey: "willowpillows::5_tandraus",
    };
  }

  if (mode === "c_moons_smhq") {
    return {
      summary_id: "preset::c_moons_smhq",
      name: "C.Moons Mods",
      subtitle: "",
      activeTags: ["C.Moons", "SMHQ"],
      iconKey: "willowpillows::5_tandraus",
    };
  }

  return null;
}

function getPresetModulePriority(mod, activeTags) {
  if (String(mod?.dev ?? "").toLowerCase() !== "tomatobird") return 1;
  const name = String(mod?.name ?? "").toLowerCase();
  const tags = Array.isArray(activeTags) ? activeTags : [];

  if (tags.includes("Brutal") && name === "bcmhqmodule") return 0;
  if (tags.includes("Wesley") && name === "wesleysmoonshqmodule") return 0;
  if (tags.includes("C.Moons") && name === "classicmoonshqmodule") return 0;

  return 1;
}

function SkeletonBlock({ className }) {
  return (
    <div
      className={cn(
        "animate-pulse rounded-xl border border-panel-outline bg-white/[0.06]",
        className
      )}
    />
  );
}

function ScrollableDropdownContent({
  className,
  scrollAreaClassName,
  children,
  ...props
}) {
  const scrollRef = useRef(null);
  const hideTimerRef = useRef(null);
  const [scrollState, setScrollState] = useState({
    visible: false,
    scrollable: false,
    thumbHeight: 0,
    thumbOffset: 0,
  });

  const showScrollbar = useCallback(() => {
    setScrollState((prev) => ({ ...prev, visible: true }));
    if (hideTimerRef.current) {
      window.clearTimeout(hideTimerRef.current);
    }
    hideTimerRef.current = window.setTimeout(() => {
      setScrollState((prev) => ({ ...prev, visible: false }));
      hideTimerRef.current = null;
    }, 500);
  }, []);

  const syncScrollbar = useCallback((shouldFlash = false) => {
    const el = scrollRef.current;
    if (!el) return;

    const scrollable = el.scrollHeight > el.clientHeight + 1;
    if (!scrollable) {
      setScrollState({
        visible: false,
        scrollable: false,
        thumbHeight: 0,
        thumbOffset: 0,
      });
      return;
    }

    const trackInset = 4;
    const trackHeight = Math.max(0, el.clientHeight - trackInset * 2);
    const thumbHeight = Math.max(
      24,
      Math.round((el.clientHeight / el.scrollHeight) * trackHeight),
    );
    const maxScroll = Math.max(1, el.scrollHeight - el.clientHeight);
    const maxOffset = Math.max(0, trackHeight - thumbHeight);
    const thumbOffset =
      trackInset + Math.round((el.scrollTop / maxScroll) * maxOffset);

    setScrollState((prev) => ({
      visible: shouldFlash ? true : prev.visible,
      scrollable: true,
      thumbHeight,
      thumbOffset,
    }));

    if (shouldFlash) {
      showScrollbar();
    }
  }, [showScrollbar]);

  useEffect(() => {
    syncScrollbar(true);
    const el = scrollRef.current;
    if (!el || typeof ResizeObserver === "undefined") {
      return () => {
        if (hideTimerRef.current) {
          window.clearTimeout(hideTimerRef.current);
          hideTimerRef.current = null;
        }
      };
    }

    const observer = new ResizeObserver(() => {
      syncScrollbar(false);
    });
    observer.observe(el);

    return () => {
      observer.disconnect();
      if (hideTimerRef.current) {
        window.clearTimeout(hideTimerRef.current);
        hideTimerRef.current = null;
      }
    };
  }, [syncScrollbar]);

  return (
    <DropdownMenu.Content
      className={cn(
        "relative overflow-hidden",
        className,
      )}
      {...props}
    >
      <div
        ref={scrollRef}
        onScroll={() => syncScrollbar(true)}
        className={cn(
          "dropdown-scroll-area overflow-y-auto",
          scrollAreaClassName,
        )}
      >
        {children}
      </div>
      {scrollState.scrollable ? (
        <div
          className={cn(
            "pointer-events-none absolute bottom-1 right-1 top-1 w-1 transition-opacity duration-150",
            scrollState.visible ? "opacity-100" : "opacity-0",
          )}
        >
          <div
            className="absolute right-0 w-1 rounded-full bg-white/35"
            style={{
              height: `${scrollState.thumbHeight}px`,
              transform: `translateY(${scrollState.thumbOffset}px)`,
            }}
          />
        </div>
      ) : null}
    </DropdownMenu.Content>
  );
}

function ModCover({ src, initials }) {
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    setFailed(false);
  }, [src]);

  if (src && !failed) {
    return (
      <div className="flex h-14 w-14 shrink-0 items-center justify-center overflow-hidden rounded-xl border border-panel-outline bg-black/25">
        <img
          src={src}
          alt=""
          className="h-full w-full object-contain"
          loading="lazy"
          onError={() => setFailed(true)}
        />
      </div>
    );
  }

  return (
    <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-2xl border border-panel-outline bg-white/10 text-base font-bold text-white/80">
      {initials}
    </div>
  );
}

function LauncherPageSkeleton({ statusText }) {
  return (
    <>
      <div className="flex flex-wrap items-center gap-3">
        <SkeletonBlock className="h-11 w-[220px] rounded-xl" />
        <SkeletonBlock className="h-11 w-28 rounded-xl" />
        <SkeletonBlock className="h-11 min-w-[220px] flex-1 rounded-xl" />
        <SkeletonBlock className="h-11 w-11 rounded-xl" />
      </div>

      <div className="rounded-2xl border border-panel-outline bg-white/5 px-4 py-3">
        <div className="flex items-center gap-2 text-sm font-medium text-white/85">
          <LoaderCircle className="h-4 w-4 animate-spin text-white/65" />
          <span>Preparing launcher</span>
        </div>
        <div className="mt-1 text-xs text-white/50">
          {statusText || "Loading local versions and mod manifest..."}
        </div>
      </div>

      <div className="grid min-h-0 flex-1 grid-cols-1 gap-4">
        <div className="min-h-0 rounded-2xl border border-panel-outline bg-white/5 p-3">
          <div className="mb-3 flex items-center justify-between px-1">
            <SkeletonBlock className="h-4 w-20 rounded-md" />
            <SkeletonBlock className="h-4 w-14 rounded-md" />
          </div>

          <div className="flex flex-col gap-2">
            {Array.from({ length: 6 }).map((_, index) => (
              <div
                key={index}
                className="flex items-start gap-3 rounded-2xl border border-panel-outline bg-black/10 px-3 py-3"
              >
                <SkeletonBlock className="h-11 w-11 shrink-0 rounded-xl" />
                <div className="min-w-0 flex-1 space-y-2">
                  <SkeletonBlock className="h-5 w-36 rounded-md" />
                  <SkeletonBlock className="h-4 w-24 rounded-md" />
                  <SkeletonBlock className="h-4 w-full rounded-md" />
                </div>
                <SkeletonBlock className="mt-2 h-6 w-10 shrink-0 rounded-full" />
              </div>
            ))}
          </div>
        </div>
      </div>
    </>
  );
}

export default function LauncherPage({
  loginState,
  onLogout,
  onRequireLogin,
  bootstrapError,
  onInstalledVersionsChange,
}) {
  const [installedVersions, setInstalledVersions] = useState([]);
  const [selectedVersion, setSelectedVersion] = useState(null);
  const [manifest, setManifest] = useState({
    version: null,
    mods: [],
    manifests: {},
  });
  const [practiceMods, setPracticeMods] = useState([]);

  const [query, setQuery] = useState("");
  const [selectedMod, setSelectedMod] = useState(null);
  const [modEnabled, setModEnabled] = useState(true);
  const [modToggleBusy, setModToggleBusy] = useState(false);
  const [modToggleBusyKeys, setModToggleBusyKeys] = useState(() => new Set());
  const [disabledMods, setDisabledMods] = useState([]); // [{dev,name}] normalized by backend
  const [installedModVersionsByVersion, setInstalledModVersionsByVersion] =
    useState({}); // version -> { key(dev::name lower) -> version }
  const [installedModIconsByVersion, setInstalledModIconsByVersion] =
    useState({}); // version -> { key(dev::name lower) -> icon path }
  const [installedModDescriptionsByVersion, setInstalledModDescriptionsByVersion] =
    useState({}); // version -> { key(dev::name lower) -> description }
  const [modCfgFilesByKey, setModCfgFilesByKey] = useState({}); // key(dev::name lower) -> ["foo.cfg", ...]

  // Download confirm modal (for non-installed versions)
  const [downloadPrompt, setDownloadPrompt] = useState({
    open: false,
    version: null,
  });

  const [updatePrompt, setUpdatePrompt] = useState({ open: false });
  const [manifestUpdateInfo, setManifestUpdateInfo] = useState(null);
  const [practicePrompt, setPracticePrompt] = useState({ open: false });
  const [practiceTask, setPracticeTask] = useState(null); // last Practice Mods progress payload
  const [practiceCancelBusy, setPracticeCancelBusy] = useState(false);
  const [presetPrompt, setPresetPrompt] = useState({ open: false });
  const [presetTask, setPresetTask] = useState(null); // last Preset Mods progress payload
  const [presetCancelBusy, setPresetCancelBusy] = useState(false);

  const [checkUpdatePrompt, setCheckUpdatePrompt] = useState({
    open: false,
    mods: [],
  });
  const [steamOverlayDialogOpen, setSteamOverlayDialogOpen] = useState(false);
  const [steamOverlayConfig, setSteamOverlayConfig] = useState({
    enabled: false,
    steam_path: "",
  });
  const [steamOverlayResolvedPath, setSteamOverlayResolvedPath] = useState("");
  const [steamOverlaySaveBusy, setSteamOverlaySaveBusy] = useState(false);
  const [steamOverlayError, setSteamOverlayError] = useState("");
  const [steamOverlaySaved, setSteamOverlaySaved] = useState("");
  const [deleteVersionPrompt, setDeleteVersionPrompt] = useState(
    makeDeleteVersionPromptState()
  );
  const [deleteVersionBusy, setDeleteVersionBusy] = useState(false);

  // Config editor state (shared config via junction) - BepInEx cfg UI
  const [configFiles, setConfigFiles] = useState([]);
  const [activeConfigPath, setActiveConfigPath] = useState("");
  const [cfgFile, setCfgFile] = useState(null); // parsed FileData
  const [activeSection, setActiveSection] = useState("");
  const [cfgError, setCfgError] = useState("");
  const [savingEntry, setSavingEntry] = useState(null); // `${section}/${entry}`
  const [configLinkEpoch, setConfigLinkEpoch] = useState(0);
  const [configLinkState, setConfigLinkState] = useState(null);

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
    run_mode: null,
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
  const [runMode, setRunMode] = useState(getInitialRunMode); // hq | practice | brutal | brutal_practice | wesley | wesley_practice | smhq
  const [launchBusy, setLaunchBusy] = useState(false);
  const [modPanelWidthPercent, setModPanelWidthPercent] = useState(() => {
    if (typeof window === "undefined") return DEFAULT_MOD_PANEL_WIDTH;
    const saved = Number(localStorage.getItem(MOD_PANEL_WIDTH_STORAGE_KEY));
    return Number.isFinite(saved)
      ? clamp(saved, 30, 70)
      : DEFAULT_MOD_PANEL_WIDTH;
  });
  const [isResizingPanels, setIsResizingPanels] = useState(false);
  const lastAutoCheckedContextRef = useRef("");
  const startupManifestSyncRef = useRef("");
  const [preparedUpdateContext, setPreparedUpdateContext] = useState("");
  const [didFinishBootstrap, setDidFinishBootstrap] = useState(false);
  const [bootstrapStatus, setBootstrapStatus] = useState(
    "Checking installed versions..."
  );
  const [modContextMenu, setModContextMenu] = useState({
    open: false,
    x: 0,
    y: 0,
    mod: null,
    configPath: "",
  });
  const [versionContextMenu, setVersionContextMenu] = useState({
    open: false,
    x: 0,
    y: 0,
    version: null,
  });
  const practicePromptOpenRef = useRef(false);
  const presetPromptOpenRef = useRef(false);
  const practiceTaskRef = useRef(null);
  const presetTaskRef = useRef(null);
  const runModeRef = useRef(runMode);
  const selectedVersionRef = useRef(selectedVersion);
  const modContextMenuRef = useRef(null);
  const versionContextMenuRef = useRef(null);
  const splitContainerRef = useRef(null);

  function updateInstalledVersionsState(nextVersions) {
    const normalized = Array.isArray(nextVersions) ? nextVersions : [];
    setInstalledVersions(normalized);
    onInstalledVersionsChange?.(normalized);
  }

  useEffect(() => {
    practicePromptOpenRef.current = !!practicePrompt.open;
  }, [practicePrompt.open]);
  useEffect(() => {
    presetPromptOpenRef.current = !!presetPrompt.open;
  }, [presetPrompt.open]);
  useEffect(() => {
    practiceTaskRef.current = practiceTask;
  }, [practiceTask]);
  useEffect(() => {
    presetTaskRef.current = presetTask;
  }, [presetTask]);
  useEffect(() => {
    runModeRef.current = runMode;
  }, [runMode]);
  useEffect(() => {
    selectedVersionRef.current = selectedVersion;
  }, [selectedVersion]);
  useEffect(() => {
    if (gameStatus.running) {
      setLaunchBusy(false);
    }
  }, [gameStatus.running]);

  useEffect(() => {
    if (typeof window === "undefined") return;
    localStorage.setItem(
      MOD_PANEL_WIDTH_STORAGE_KEY,
      String(modPanelWidthPercent)
    );
  }, [modPanelWidthPercent]);

  useEffect(() => {
    let cancelled = false;
    invoke("get_steam_overlay_config")
      .then((cfg) => {
        if (cancelled) return;
        const resolvedPath = String(cfg?.resolved_steam_path ?? "");
        setSteamOverlayResolvedPath(resolvedPath);
        setSteamOverlayConfig({
          enabled: !!cfg?.enabled,
          steam_path: String(cfg?.steam_path ?? cfg?.resolved_steam_path ?? ""),
        });
      })
      .catch((error) => {
        if (cancelled) return;
        setSteamOverlayError(error?.message ?? String(error));
      });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!isResizingPanels) return;

    const prevCursor = document.body.style.cursor;
    const prevUserSelect = document.body.style.userSelect;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";

    return () => {
      document.body.style.cursor = prevCursor;
      document.body.style.userSelect = prevUserSelect;
    };
  }, [isResizingPanels]);

  useEffect(() => {
    if (!modContextMenu.open) return;

    const close = () =>
      setModContextMenu((prev) => ({ ...prev, open: false, mod: null }));

    const handlePointerDown = (event) => {
      if (modContextMenuRef.current?.contains(event.target)) return;
      close();
    };

    const handleEscape = (event) => {
      if (event.key === "Escape") close();
    };

    const handleWindowChange = () => close();

    const adjustPosition = () => {
      const menu = modContextMenuRef.current;
      if (!menu) return;
      const rect = menu.getBoundingClientRect();
      const nextX = Math.min(
        modContextMenu.x,
        Math.max(8, window.innerWidth - rect.width - 8)
      );
      const nextY = Math.min(
        modContextMenu.y,
        Math.max(8, window.innerHeight - rect.height - 8)
      );
      if (nextX !== modContextMenu.x || nextY !== modContextMenu.y) {
        setModContextMenu((prev) => ({ ...prev, x: nextX, y: nextY }));
      }
    };

    const raf = window.requestAnimationFrame(adjustPosition);
    window.addEventListener("pointerdown", handlePointerDown);
    window.addEventListener("keydown", handleEscape);
    window.addEventListener("resize", handleWindowChange);
    window.addEventListener("scroll", handleWindowChange, true);

    return () => {
      window.cancelAnimationFrame(raf);
      window.removeEventListener("pointerdown", handlePointerDown);
      window.removeEventListener("keydown", handleEscape);
      window.removeEventListener("resize", handleWindowChange);
      window.removeEventListener("scroll", handleWindowChange, true);
    };
  }, [modContextMenu.open, modContextMenu.x, modContextMenu.y]);

  useEffect(() => {
    if (!versionContextMenu.open) return;

    const close = () =>
      setVersionContextMenu((prev) => ({ ...prev, open: false, version: null }));

    const handlePointerDown = (event) => {
      if (versionContextMenuRef.current?.contains(event.target)) return;
      close();
    };

    const handleEscape = (event) => {
      if (event.key === "Escape") close();
    };

    const handleWindowChange = () => close();

    const adjustPosition = () => {
      const menu = versionContextMenuRef.current;
      if (!menu) return;
      const rect = menu.getBoundingClientRect();
      const nextX = Math.min(
        versionContextMenu.x,
        Math.max(8, window.innerWidth - rect.width - 8)
      );
      const nextY = Math.min(
        versionContextMenu.y,
        Math.max(8, window.innerHeight - rect.height - 8)
      );
      if (nextX !== versionContextMenu.x || nextY !== versionContextMenu.y) {
        setVersionContextMenu((prev) => ({ ...prev, x: nextX, y: nextY }));
      }
    };

    const raf = window.requestAnimationFrame(adjustPosition);
    window.addEventListener("pointerdown", handlePointerDown);
    window.addEventListener("keydown", handleEscape);
    window.addEventListener("resize", handleWindowChange);
    window.addEventListener("scroll", handleWindowChange, true);

    return () => {
      window.cancelAnimationFrame(raf);
      window.removeEventListener("pointerdown", handlePointerDown);
      window.removeEventListener("keydown", handleEscape);
      window.removeEventListener("resize", handleWindowChange);
      window.removeEventListener("scroll", handleWindowChange, true);
    };
  }, [versionContextMenu.open, versionContextMenu.x, versionContextMenu.y]);

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

  const installedModIcons = useMemo(() => {
    const v = Number(selectedVersion);
    if (!Number.isFinite(v)) return {};
    const byV = installedModIconsByVersion?.[v];
    return byV && typeof byV === "object" ? byV : {};
  }, [installedModIconsByVersion, selectedVersion]);

  const installedModIconUrls = useMemo(() => {
    const out = {};
    for (const [key, path] of Object.entries(installedModIcons)) {
      if (typeof path === "string" && path) {
        out[key] = convertFileSrc(path);
      }
    }
    return out;
  }, [installedModIcons]);

  const installedModDescriptions = useMemo(() => {
    const v = Number(selectedVersion);
    if (!Number.isFinite(v)) return {};
    const byV = installedModDescriptionsByVersion?.[v];
    return byV && typeof byV === "object" ? byV : {};
  }, [installedModDescriptionsByVersion, selectedVersion]);

  const presetSummaryEntry = useMemo(() => {
    const spec = getPresetSummarySpec(runMode);
    if (!spec) return null;

    const taggedMods = (Array.isArray(manifest.mods) ? manifest.mods : [])
      .filter(
        (mod) =>
          !isUiHiddenMod(mod) &&
          !SMHQ_FORCED_MOD_KEYS.has(modKeyLower(mod)) &&
          isModCompatibleWithTags(mod, selectedVersion, spec.activeTags)
      )
      .sort((a, b) => {
        return (
          getPresetModulePriority(a, spec.activeTags) -
          getPresetModulePriority(b, spec.activeTags)
        );
      });

    const installedCount = taggedMods.filter(
      (mod) => !!installedModVersions[modKeyLower(mod)]
    ).length;

    return {
      isPresetSummary: true,
      summary_id: spec.summary_id,
      name: spec.name,
      dev: spec.subtitle,
      description:
        taggedMods.length > 0
          ? `${taggedMods.length} mods installed by this preset.`
          : "mods installed by this preset.",
      summaryTags: spec.activeTags,
      summaryItems: taggedMods,
      totalCount: taggedMods.length,
      installedCount,
      iconSrc: installedModIconUrls[spec.iconKey] ?? null,
    };
  }, [
    installedModIconUrls,
    installedModVersions,
    manifest.mods,
    runMode,
    selectedVersion,
  ]);

  const practiceReferenceMods = useMemo(() => {
    const mods = Array.isArray(practiceMods)
      ? practiceMods
      : [];
    return mods.filter(
      (m) => !isUiHiddenMod(m) && isModCompatibleWithVersion(m, selectedVersion)
    );
  }, [practiceMods, selectedVersion]);

  const smhqReferenceMods = useMemo(() => {
    if (!isSmhqRunMode(runMode)) return [];
    const mods = Array.isArray(manifest.mods) ? manifest.mods : [];
    return mods.filter((m) => {
      const key = modKeyLower(m);
      return (
        SMHQ_FORCED_MOD_KEYS.has(key) &&
        m?.enabled !== false &&
        !isUiHiddenMod(m) &&
        isModCompatibleWithVersion(m, selectedVersion)
      );
    });
  }, [manifest.mods, runMode, selectedVersion]);

  const modsForList = useMemo(() => {
    const regularMods = (Array.isArray(manifest.mods) ? manifest.mods : []).filter(
      (m) =>
        !(Array.isArray(m?.tags) && m.tags.length > 0) &&
        m?.enabled !== false &&
        isModCompatibleWithVersion(m, selectedVersion)
    );
    const merged = [
      ...(isPracticeRunMode(runMode) ? practiceReferenceMods : []),
      ...smhqReferenceMods,
      ...regularMods,
    ];
    if (!isPracticeRunMode(runMode) && !isSmhqRunMode(runMode)) return regularMods;

    const seen = new Set();
    return merged.filter((m) => {
      const key = modKeyLower(m);
      if (!key || seen.has(key)) return false;
      seen.add(key);
      return true;
    });
  }, [manifest.mods, practiceReferenceMods, runMode, selectedVersion, smhqReferenceMods]);

  const availableModKeys = useMemo(
    () => new Set(modsForList.map((m) => modKeyLower(m))),
    [modsForList]
  );

  const practiceModKeys = useMemo(
    () => new Set(practiceReferenceMods.map((m) => modKeyLower(m))),
    [practiceReferenceMods]
  );

  const practiceLockedModKeys = useMemo(() => {
    if (!isPracticeRunMode(runMode)) return new Set();
    return PRACTICE_LOCKED_MOD_KEYS;
  }, [runMode]);

  const smhqForcedModKeys = useMemo(() => {
    if (!isSmhqRunMode(runMode)) return new Set();
    return new Set(smhqReferenceMods.map((m) => modKeyLower(m)));
  }, [runMode, smhqReferenceMods]);

  useEffect(() => {
    if (!isPracticeRunMode(runMode)) return;
    console.log("[practice-debug]", {
      runMode,
      selectedVersion,
      practiceModsFromCommand: practiceMods.map((m) => modKeyLower(m)),
      practiceModsForVersion: practiceReferenceMods.map((m) => modKeyLower(m)),
      installedModKeys: Object.keys(installedModVersions),
      matchedPracticeMods: practiceReferenceMods
        .filter((m) => !!installedModVersions[modKeyLower(m)])
        .map((m) => modKeyLower(m)),
      modsForListHead: modsForList.slice(0, 10).map((m) => modKeyLower(m)),
    });
  }, [
    runMode,
    selectedVersion,
    practiceMods,
    practiceReferenceMods,
    installedModVersions,
    modsForList,
  ]);

  const filteredMods = useMemo(() => {
    const q = query.trim().toLowerCase();
    const mods = modsForList;

    const selectedKeyLower = selectedMod
      ? modKeyLower(selectedMod)
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
      const cfgs = modCfgFilesByKey[`${selectedVersion}::${keyLower}`];
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

    // 1) Build chain groups using ALL visible mods (so search can match any member,
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
    modsForList,
    manifest.chain_config,
    query,
    selectedMod,
    selectedVersion,
    modCfgFilesByKey,
    installedModVersions,
  ]);

  const displayedMods = useMemo(() => {
    if (!presetSummaryEntry) return filteredMods;
    return [presetSummaryEntry, ...filteredMods];
  }, [filteredMods, presetSummaryEntry]);

  // If a tagged/preset-only mod was selected (e.g. before this filtering), clear the selection.
  useEffect(() => {
    if (!selectedMod) return;
    if (isPresetSummaryMod(selectedMod)) {
      if (!presetSummaryEntry || selectedMod.summary_id !== presetSummaryEntry.summary_id) {
        setSelectedMod(null);
      }
      return;
    }
    if (!availableModKeys.has(modKeyLower(selectedMod))) {
      setSelectedMod(null);
    }
  }, [selectedMod, availableModKeys, presetSummaryEntry]);

  // Best-effort prefetch of config-file matches per mod so chain-dedup is accurate.
  useEffect(() => {
    const mods = modsForList;
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
        .filter(
          (x) =>
            x.keyLower &&
            modCfgFilesByKey[`${selectedVersion}::${x.keyLower}`] == null
        );

      if (queue.length === 0) return;

      const nextMap = { ...modCfgFilesByKey };
      let idx = 0;
      async function worker() {
        while (idx < queue.length && !cancelled) {
          const cur = queue[idx++];
          try {
            const files = await invoke("list_config_files_for_mod_for_version", {
              version: selectedVersion,
              dev: cur.mod.dev,
              name: cur.mod.name,
            });
            nextMap[`${selectedVersion}::${cur.keyLower}`] = (
              Array.isArray(files) ? files : []
            )
              .map((p) => String(p))
              .filter((p) => p.toLowerCase().endsWith(".cfg"));
          } catch {
            nextMap[`${selectedVersion}::${cur.keyLower}`] = [];
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
  }, [modsForList, selectedVersion, configLinkEpoch]);

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
    let cancelled = false;

    (async () => {
      const saved = localStorage.getItem("selectedVersion");
      const savedNum = saved == null ? null : Number(saved);

      const manifestPromise = invoke("get_manifest");
      const practiceModsPromise = invoke("get_practice_mod_list");
      setBootstrapStatus("Checking installed versions...");

      let vList = [];
      try {
        const versions = await invoke("list_installed_versions");
        vList = Array.isArray(versions) ? versions : [];
      } catch (e) {
        console.error(e);
      }

      if (cancelled) return;

      updateInstalledVersionsState(vList);

      // Pick a usable version immediately so the UI does not wait on the remote manifest.
      if (Number.isFinite(savedNum) && (vList.length === 0 || vList.includes(savedNum))) {
        setSelectedVersion(savedNum);
      } else if (vList.length > 0) {
        setSelectedVersion(vList[vList.length - 1]);
      } else if (Number.isFinite(savedNum)) {
        setSelectedVersion(savedNum);
      }

      // initial running status
      try {
        setBootstrapStatus("Checking game status...");
        const s = await invoke("get_game_status");
        if (!cancelled) {
          setGameStatus(s ?? { running: false, pid: null });
        }
      } catch {}

      // disabled mods list
      try {
        setBootstrapStatus("Loading mod preferences...");
        const dm = await invoke("get_disabled_mods");
        if (!cancelled) {
          setDisabledMods(Array.isArray(dm) ? dm : []);
        }
      } catch {}

      try {
        setBootstrapStatus("Fetching remote mod manifest...");
        const mf = await manifestPromise;
        if (cancelled) return;

        setManifest(
          mf ?? { version: null, mods: [], manifests: {}, preset_tag_constraints: {} }
        );

        const remoteV =
          mf?.manifests && typeof mf.manifests === "object"
            ? Object.keys(mf.manifests)
                .map((k) => Number(k))
                .filter((n) => Number.isFinite(n))
            : [];
        remoteV.sort((a, b) => a - b);

        const availableVersions = new Set([...vList, ...remoteV]);
        setSelectedVersion((prev) => {
          const prevNum = Number(prev);
          if (Number.isFinite(prevNum) && availableVersions.has(prevNum)) {
            return prevNum;
          }
          if (Number.isFinite(savedNum) && availableVersions.has(savedNum)) {
            return savedNum;
          }
          if (vList.length > 0) return vList[vList.length - 1];
          if (remoteV.length > 0) return remoteV[remoteV.length - 1];
          return Number.isFinite(prevNum) ? prevNum : null;
        });
      } catch (e) {
        console.error(e);
      } finally {
        if (!cancelled) {
          setBootstrapStatus("Finalizing launcher...");
          setDidFinishBootstrap(true);
        }
      }

      try {
        const pm = await practiceModsPromise;
        if (!cancelled) {
          setPracticeMods(Array.isArray(pm) ? pm : []);
        }
      } catch (e) {
        console.error(e);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [onInstalledVersionsChange]);

  // Let the Titlebar know what version is currently selected
  useEffect(() => {
    const v = Number(selectedVersion);
    if (!Number.isFinite(v)) return;
    emit("ui://selected-version-changed", { version: v }).catch(() => {});
  }, [selectedVersion]);

  async function refreshInstalledModVersions(v = selectedVersion, opts = {}) {
    const vv = Number(v);
    if (!Number.isFinite(vv)) return;
    if (!isInstalled(v)) {
      setInstalledModVersionsByVersion((prev) => ({ ...prev, [vv]: {} }));
      setInstalledModIconsByVersion((prev) => ({ ...prev, [vv]: {} }));
      setInstalledModDescriptionsByVersion((prev) => ({ ...prev, [vv]: {} }));
      return {};
    }
    const retries = Math.max(0, Number(opts?.retries ?? 0));
    const delayMs = Math.max(0, Number(opts?.delayMs ?? 250));
    const expectedKeys = Array.isArray(opts?.expectedKeys)
      ? opts.expectedKeys.filter(Boolean)
      : [];

    for (let attempt = 0; attempt <= retries; attempt += 1) {
      try {
      const list = await invoke("list_installed_mod_versions", { version: v });
      const map = {};
      const iconMap = {};
      const descriptionMap = {};
      for (const it of Array.isArray(list) ? list : []) {
        const k = `${String(it.dev).toLowerCase()}::${String(
          it.name
        ).toLowerCase()}`;
        map[k] = String(it.version ?? "");
        if (typeof it.icon_path === "string" && it.icon_path) {
          iconMap[k] = it.icon_path;
        }
        if (typeof it.description === "string" && it.description.trim()) {
          descriptionMap[k] = it.description.trim();
        }
      }
      setInstalledModVersionsByVersion((prev) => ({ ...prev, [vv]: map }));
      setInstalledModIconsByVersion((prev) => ({ ...prev, [vv]: iconMap }));
      setInstalledModDescriptionsByVersion((prev) => ({
        ...prev,
        [vv]: descriptionMap,
      }));
        const missingExpected =
          expectedKeys.length > 0 && expectedKeys.every((key) => !map[key]);
        if (!missingExpected || attempt === retries) {
          return map;
        }
      } catch {
        if (attempt === retries) {
          // best-effort (missing folder, etc)
          setInstalledModVersionsByVersion((prev) => ({ ...prev, [vv]: {} }));
          setInstalledModIconsByVersion((prev) => ({ ...prev, [vv]: {} }));
          setInstalledModDescriptionsByVersion((prev) => ({ ...prev, [vv]: {} }));
          return {};
        }
      }
      await sleep(delayMs);
    }
  }

  // auto-run update check once the local mod state is ready for this run context
  useEffect(() => {
    if (!didFinishBootstrap) return;
    const version = Number(selectedVersion);
    if (!Number.isFinite(version)) return;
    if (!isInstalled(version)) {
      setManifestUpdateInfo(null);
      return;
    }
    const checkKey = `manifest:${version}`;
    if (startupManifestSyncRef.current === checkKey) return;

    startupManifestSyncRef.current = checkKey;

    (async () => {
      try {
        const info = await invoke("check_latest_install_manifest_update", { version });
        if (info?.available) {
          setTask((t) => ({
            ...t,
            status: "idle",
            version,
            step_name: null,
            steps_total: null,
            step: null,
            overall_percent: null,
            detail: null,
            downloaded_bytes: null,
            total_bytes: null,
            error: null,
          }));
          setManifestUpdateInfo(info);
          setUpdatePrompt({ open: true });
        } else if (manifestUpdateInfo?.version === version) {
          setManifestUpdateInfo(null);
        }
      } catch (e) {
        setUpdatePrompt({ open: true });
        setTask((t) => ({
          ...t,
          status: "error",
          error: e?.message ?? String(e),
        }));
      }
    })();
  }, [didFinishBootstrap, installedVersions, isInstalled, selectedVersion]);

  useEffect(() => {
    if (!didFinishBootstrap) return;
    const version = Number(selectedVersion);
    if (!Number.isFinite(version)) return;
    if (!isInstalled(version)) return;
    const contextKey = `${runMode}:${version}`;
    if (preparedUpdateContext !== contextKey) return;
    if (lastAutoCheckedContextRef.current === contextKey) return;
    lastAutoCheckedContextRef.current = contextKey;
    checkModUpdates(version, { runMode });
  }, [
    didFinishBootstrap,
    installedVersions,
    isInstalled,
    preparedUpdateContext,
    runMode,
    selectedVersion,
  ]);

  // refresh installed plugin versions when selected version changes
  useEffect(() => {
    refreshInstalledModVersions(selectedVersion);
  }, [selectedVersion, installedVersions]);

  // Startup can race with plugin file discovery, especially for practice-related mods.
  // Do a short delayed re-scan so icons/descriptions catch up after initial bootstrap.
  useEffect(() => {
    if (!didFinishBootstrap) return;
    const version = Number(selectedVersion);
    if (!Number.isFinite(version)) return;
    if (!isInstalled(version)) return;

    const timeoutId = setTimeout(() => {
      refreshInstalledModVersions(version, {
        retries: isPracticeRunMode(runMode) ? 6 : 2,
        delayMs: 300,
      }).catch(() => {});
    }, 350);

    return () => clearTimeout(timeoutId);
  }, [
    didFinishBootstrap,
    isInstalled,
    runMode,
    selectedVersion,
    practiceMods,
  ]);

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
        const p = event?.payload ?? {};
        const totalFiles = Number(p?.total_files ?? 0);
        const extractedFiles = Number(p?.extracted_files ?? 0);
        const stepProgress = Number(p?.step_progress ?? 0);
        const overall = Number(p?.overall_percent ?? 0);
        const stepName = String(p?.step_name ?? "");
        const isEnableModStep = stepName === "Enable Mod";
        const isModFilesStep = stepName === "Mod Files";
        const isDeleteStep = stepName === "Delete Version";
        const isManifestSyncStep =
          stepName === "Sync Mods" || stepName === "Sync Game";
        const isSetupStep =
          isEnableModStep ||
          stepName === "Practice Mods" ||
          stepName === "Preset Mods" ||
          isModFilesStep ||
          isDeleteStep;
        const didFinish =
          (Number.isFinite(totalFiles) &&
            totalFiles > 0 &&
            Number.isFinite(extractedFiles) &&
            extractedFiles >= totalFiles) ||
          (Number.isFinite(stepProgress) && stepProgress >= 1) ||
          (Number.isFinite(overall) && overall >= 100);

        // Practice install modal: only show when practice actually installs missing plugins.
        if (stepName === "Practice Mods") {
          setPracticeCancelBusy(false);
          setPracticeTask({
            status: didFinish ? "done" : "working",
            ...p,
            error: null,
          });
          if (Number.isFinite(totalFiles) && totalFiles > 0) {
            setPracticePrompt({ open: true });
          }
        }
        // Preset install modal (tagged mods like Brutal/Wesley).
        if (stepName === "Preset Mods") {
          setPresetCancelBusy(false);
          setPresetTask({
            status: didFinish ? "done" : "working",
            ...p,
            error: null,
          });
          if (Number.isFinite(totalFiles) && totalFiles > 0) {
            setPresetPrompt({ open: true });
          }
        }
        if (isModFilesStep) {
          const nextTask = {
            status: didFinish ? "done" : "working",
            ...p,
            error: null,
          };
          if (isPracticeRunMode(runMode)) {
            setPracticeCancelBusy(false);
            setPracticeTask(nextTask);
            if (Number.isFinite(totalFiles) && totalFiles > 0) {
              setPracticePrompt({ open: true });
            }
          } else {
            setPresetCancelBusy(false);
            setPresetTask(nextTask);
            if (Number.isFinite(totalFiles) && totalFiles > 0) {
              setPresetPrompt({ open: true });
            }
          }
        }
        if (isDeleteStep) {
          setDeleteVersionPrompt((prev) =>
            makeDeleteVersionPromptState({
              ...prev,
              open: true,
              version: Number.isFinite(Number(p?.version))
                ? Number(p.version)
                : prev.version,
              status: didFinish ? "done" : "working",
              overall_percent: Number.isFinite(overall) ? overall : 0,
              detail: String(p?.detail ?? ""),
              deleted_files: Number.isFinite(extractedFiles) ? extractedFiles : 0,
              total_files: Number.isFinite(totalFiles) ? totalFiles : 0,
              error: "",
            })
          );
        }
        if (isEnableModStep) {
          setUpdatePrompt({ open: true });
          setTask((t) => ({
            ...t,
            status: didFinish ? "done" : "working",
            ...p,
            error: null,
          }));
        }
        if (isManifestSyncStep) {
          setUpdatePrompt({ open: true });
          setTask((t) => ({
            ...t,
            status: didFinish ? "done" : "working",
            ...p,
            error: null,
          }));
        }
        // IMPORTANT: Keep preset/practice setup progress OUT of the download prompt state.
        // Otherwise the "Download version" modal can get stuck showing setup progress.
        if (!isSetupStep) {
          setTask((t) => ({
            ...t,
            status: "working",
            ...p,
            error: null,
          }));
        }
      });
      unlistenFinished = await listen("download://finished", (event) => {
        setTask((t) => ({
          ...t,
          status: "done",
          ...event.payload,
        }));
        // refresh installed versions list after install
        invoke("list_installed_versions")
          .then((v) => updateInstalledVersionsState(v))
          .catch(() => {});
        // refresh installed plugin versions for this game version
        const v = Number(event.payload?.version);
        if (Number.isFinite(v)) {
          refreshInstalledModVersions(v);
          refreshConfigLinkState(v);
        }
      });
      unlistenError = await listen("download://error", (event) => {
        // If practice setup fails, keep the modal open and show the error.
        if (
          practicePromptOpenRef.current &&
          practiceTaskRef.current
        ) {
          setPracticeCancelBusy(false);
          setPracticeTask((t) => ({
            ...(t ?? {}),
            status: "error",
            version: event.payload?.version ?? t?.version,
            error: event.payload?.message ?? "Unknown error",
          }));
        }
        // If preset setup fails, keep the modal open and show the error.
        if (
          presetPromptOpenRef.current &&
          presetTaskRef.current
        ) {
          setPresetCancelBusy(false);
          setPresetTask((t) => ({
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

  // Auto-close setup modals on success.
  useEffect(() => {
    if (!practicePrompt.open) return;
    if ((practiceTask?.status ?? "working") !== "done") return;
    const t = setTimeout(() => {
      setPracticePrompt({ open: false });
    }, 350);
    return () => clearTimeout(t);
  }, [practicePrompt.open, practiceTask?.status]);

  useEffect(() => {
    if (!presetPrompt.open) return;
    if ((presetTask?.status ?? "working") !== "done") return;
    const t = setTimeout(() => {
      setPresetPrompt({ open: false });
    }, 350);
    return () => clearTimeout(t);
  }, [presetPrompt.open, presetTask?.status]);

  // listen to backend check mods update events
  useEffect(() => {
    let unlistenCheckUpdateProgress = null;
    let unlistenCheckUpdateFinished = null;
    let unlistenCheckUpdateError = null;

    (async () => {
      unlistenCheckUpdateProgress = await listen(
        "updatable://progress",
        (event) => {
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
            run_mode: t.run_mode,
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
            run_mode: t.run_mode,
          }));
          // refresh installed versions list after install
          invoke("list_installed_versions")
            .then((v) => updateInstalledVersionsState(v))
            .catch(() => {});
        }
      );
      unlistenCheckUpdateError = await listen("updatable://error", (event) => {
        setCheckUpdateTask((t) => ({
          ...t,
          status: "error",
          version: event.payload?.version ?? t.version,
          run_mode: t.run_mode,
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

  // If link/unlink is triggered from the Titlebar while config UI is open,
  // force-refresh config lists + active file immediately.
  useEffect(() => {
    let unlisten = null;
    (async () => {
      unlisten = await listen("config://link-changed", (event) => {
        const v = Number(event?.payload?.version);
        if (Number.isFinite(v) && v !== Number(selectedVersion)) return;
        setConfigLinkEpoch((x) => x + 1);
      });
    })();
    return () => {
      if (typeof unlisten === "function") unlisten();
    };
  }, [selectedVersion]);

  async function refreshConfigLinkState(v = selectedVersion) {
    const vv = Number(v);
    if (!Number.isFinite(vv)) {
      setConfigLinkState(null);
      return null;
    }
    try {
      const s = await invoke("get_config_link_state_for_version", {
        version: vv,
      });
      if (vv === Number(selectedVersion)) {
        setConfigLinkState(s ?? null);
      }
      return s ?? null;
    } catch {
      if (vv === Number(selectedVersion)) {
        setConfigLinkState(null);
      }
      return null;
    }
  }

  // Track current link state so we can show a banner in config editor.
  useEffect(() => {
    refreshConfigLinkState(selectedVersion);
  }, [configLinkEpoch, selectedVersion]);

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

    if (isPresetSummaryMod(selectedMod)) {
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

      const files = await invoke("list_config_files_for_mod_for_version", {
        // Always use the selected version config dir; if it's linked, it's a junction to shared.
        version: selectedVersion,
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
  }, [selectedMod, selectedVersion, disabledSet, configLinkEpoch]);

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
      const parsed = await invoke("read_bepinex_cfg_for_version", {
        version: selectedVersion,
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
  }, [activeConfigPath, selectedVersion, configLinkEpoch]);

  async function downloadVersion(v, didRetryAfterLogin = false) {
    if (!loginState?.is_logged_in && typeof onRequireLogin === "function") {
      const shouldRestorePrompt =
        downloadPrompt.open && downloadPrompt.version === v;
      if (shouldRestorePrompt) {
        resetTaskForVersion(v);
        setDownloadPrompt({ open: false, version: null });
      }

      try {
        const didLogin = await onRequireLogin();
        if (!didLogin) {
          if (shouldRestorePrompt) {
            setDownloadPrompt({ open: true, version: v });
          }
          return;
        }
      } catch {
        if (shouldRestorePrompt) {
          setDownloadPrompt({ open: true, version: v });
        }
        return;
      }

      if (shouldRestorePrompt) {
        setDownloadPrompt({ open: true, version: v });
      }
    }

    setTask((t) => ({
      ...t,
      status: "working",
      version: v,
      overall_percent: 0,
      error: null,
    }));
    try {
      await invoke("download", { version: v });
      // If user already selected a preset run mode, prepare its mods right after download.
      // This keeps "Start Run" fast and avoids doing installs at launch time.
      try {
        await prepareRunMode(runMode, v, { assumeInstalled: true });
      } catch {}
    } catch (e) {
      if (
        !didRetryAfterLogin &&
        isAuthError(e) &&
        typeof onRequireLogin === "function"
      ) {
        resetTaskForVersion(v);
        const shouldRestorePrompt =
          downloadPrompt.open && downloadPrompt.version === v;
        try {
          if (shouldRestorePrompt) {
            setDownloadPrompt({ open: false, version: null });
          }
          const didLogin = await onRequireLogin();
          if (!didLogin) {
            if (shouldRestorePrompt) {
              setDownloadPrompt({ open: true, version: v });
            }
            return;
          }
          if (shouldRestorePrompt) {
            setDownloadPrompt({ open: true, version: v });
          }
          // After login, retry once automatically.
          return await downloadVersion(v, true);
        } catch {
          if (shouldRestorePrompt) {
            setDownloadPrompt({ open: true, version: v });
          }
          return;
        }
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

  async function checkModUpdates(v, opts = {}) {
    const nextRunMode =
      typeof opts?.runMode === "string" && opts.runMode
        ? opts.runMode
        : runMode;
    setCheckUpdateTask((t) => ({
      ...t,
      status: "working",
      version: v,
      run_mode: nextRunMode,
      overall_percent: 0,
      detail: null,
      updatable_mods: [],
      checked: 0,
      total: 0,
      error: null,
    }));
    try {
      await invoke("check_mod_updates", { version: v, runMode: nextRunMode });
    } catch (e) {
      setCheckUpdateTask((t) => ({
        ...t,
        status: "error",
        version: v,
        run_mode: nextRunMode,
        error: e?.message ?? String(e),
      }));
    }
  }

  async function runModUpdate(v) {
    setUpdatePrompt({ open: true });
    setManifestUpdateInfo(null);
    setTask((t) => ({
      ...t,
      status: "working",
      version: v,
      overall_percent: 0,
      error: null,
    }));
    try {
      await invoke("apply_mod_updates", { version: v, runMode: runMode });
      // Avoid re-checking (network heavy). Assume up-to-date after successful apply.
      setCheckUpdateTask((t) => ({
        ...t,
        status: "done",
        version: v,
        overall_percent: 100,
        detail: "All mod versions are synced",
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

  async function deleteVersion(version) {
    const vv = Number(version);
    if (!Number.isFinite(vv)) return;

    setDeleteVersionBusy(true);
    setDeleteVersionPrompt((prev) =>
      makeDeleteVersionPromptState({
        ...prev,
        open: true,
        version: vv,
        status: "working",
        overall_percent: 0,
        detail: "Preparing delete...",
        deleted_files: 0,
        total_files: 0,
        error: "",
      })
    );
    try {
      await invoke("delete_installed_version", { version: vv });
      const versions = await invoke("list_installed_versions");
      const nextInstalled = Array.isArray(versions) ? versions : [];
      updateInstalledVersionsState(nextInstalled);

      setInstalledModVersionsByVersion((prev) => {
        const next = { ...prev };
        delete next[vv];
        return next;
      });
      setInstalledModIconsByVersion((prev) => {
        const next = { ...prev };
        delete next[vv];
        return next;
      });
      setInstalledModDescriptionsByVersion((prev) => {
        const next = { ...prev };
        delete next[vv];
        return next;
      });
      setConfigLinkState((prev) =>
        Number(selectedVersion) === vv ? null : prev
      );

      setSelectedVersion((prev) => {
        if (Number(prev) !== vv) return prev;
        if (nextInstalled.length > 0) return nextInstalled[nextInstalled.length - 1];

        const remoteV =
          manifest?.manifests && typeof manifest.manifests === "object"
            ? Object.keys(manifest.manifests)
                .map((k) => Number(k))
                .filter((n) => Number.isFinite(n))
                .sort((a, b) => b - a)
            : [];
        return remoteV[0] ?? prev;
      });

      setDeleteVersionPrompt((prev) =>
        makeDeleteVersionPromptState({
          ...prev,
          open: true,
          version: vv,
          status: "done",
          overall_percent: 100,
          detail: prev.detail || `Deleted v${vv}.`,
          deleted_files: prev.total_files || prev.deleted_files,
          total_files: prev.total_files || prev.deleted_files,
          error: "",
        })
      );
      await sleep(180);
      setDeleteVersionPrompt(makeDeleteVersionPromptState());
      closeVersionContextMenu();
    } catch (e) {
      setDeleteVersionPrompt((prev) =>
        makeDeleteVersionPromptState({
          ...prev,
          open: true,
          version: vv,
          status: "error",
          error: e?.message ?? String(e),
        })
      );
    } finally {
      setDeleteVersionBusy(false);
    }
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
      await invoke("set_bepinex_cfg_entry_for_version", {
        version: selectedVersion,
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
        await invoke("set_bepinex_cfg_entry_for_version", {
          version: selectedVersion,
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
        const parsed = await invoke("read_bepinex_cfg_for_version", {
          version: selectedVersion,
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
      const files = await invoke("list_config_files_for_mod_for_version", {
        version: selectedVersion,
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
    const mods = modsForList;

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
    if (isPresetSummaryMod(mod)) return;
    const propagateChain = opts?.propagateChain ?? true;
    if (!isInstalled(selectedVersion)) {
      openDownloadPrompt(selectedVersion);
      return;
    }

    const baseKey = `${String(mod.dev).toLowerCase()}::${String(
      mod.name
    ).toLowerCase()}`;
    if (isPracticeRunMode(runMode) && nextEnabled && practiceLockedModKeys.has(baseKey)) {
      return;
    }
    if (isSmhqRunMode(runMode) && !nextEnabled && smhqForcedModKeys.has(baseKey)) {
      return;
    }

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
    if (isPresetSummaryMod(selectedMod)) return;
    return toggleModEnabledForMod(selectedMod, nextEnabled);
  }

  const selectedRunModeRange = useMemo(
    () => getPresetVersionRange(manifest, runMode),
    [manifest, runMode]
  );

  const versionOptions = useMemo(() => {
    const set = new Set(installedVersions);
    set.add(selectedVersion);
    if (selectedRunModeRange?.low != null) set.add(selectedRunModeRange.low);
    if (selectedRunModeRange?.high != null) set.add(selectedRunModeRange.high);
    // show versions provided by remote manifest (version -> download_manifest)
    const remoteV =
      manifest?.manifests && typeof manifest.manifests === "object"
        ? Object.keys(manifest.manifests)
            .map((k) => Number(k))
            .filter((n) => Number.isFinite(n))
        : [];
    remoteV.forEach((v) => set.add(v));
    let list = Array.from(set).sort((a, b) => b - a);
    list = list.filter((v) => isVersionWithinRange(v, selectedRunModeRange));
    return list;
  }, [installedVersions, selectedVersion, manifest, selectedRunModeRange]);

  // If the selected version is outside the allowed range for this run mode, bump it back in range.
  useEffect(() => {
    if (!Number.isFinite(Number(selectedVersion))) return;
    if (isVersionWithinRange(selectedVersion, selectedRunModeRange)) return;

    const nextV = clampVersionToRange(selectedVersion, selectedRunModeRange);
    const shouldRunSideEffects = didFinishBootstrap;
    setSelectedVersion(nextV);
    if (!shouldRunSideEffects) return;
    if (isInstalled(nextV)) {
      invoke("apply_disabled_mods", { version: nextV })
        .catch(() => {})
        .finally(() => {
          // After bumping the version, also prepare the selected run mode now.
          prepareRunMode(runMode, nextV).catch(() => {});
        });
    } else {
      openDownloadPrompt(nextV);
    }
  }, [didFinishBootstrap, selectedRunModeRange, selectedVersion, installedVersions, runMode]);

  const selectedInstalled = isInstalled(selectedVersion);
  const selectedVersionLabel = Number.isFinite(Number(selectedVersion))
    ? `v${selectedVersion}`
    : "Select version";
  const selectedPresetSummary = isPresetSummaryMod(selectedMod) ? selectedMod : null;
  const showResizablePanels = !!selectedMod;
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

  function closeModContextMenu() {
    setModContextMenu((prev) => ({
      ...prev,
      open: false,
      mod: null,
      configPath: "",
    }));
  }

  function closeVersionContextMenu() {
    setVersionContextMenu((prev) => ({ ...prev, open: false, version: null }));
  }

  async function openModContextMenu(event, mod) {
    event.preventDefault();
    const version = Number(selectedVersion);
    const keyLower = `${String(mod?.dev ?? "").toLowerCase()}::${String(
      mod?.name ?? ""
    ).toLowerCase()}`;
    let configPath = "";

    const cachedFiles = modCfgFilesByKey[`${version}::${keyLower}`];
    if (Array.isArray(cachedFiles)) {
      configPath = cachedFiles.find((p) =>
        String(p).toLowerCase().endsWith(".cfg")
      ) ?? "";
    } else if (Number.isFinite(version) && mod?.dev && mod?.name) {
      try {
        const files = await invoke("list_config_files_for_mod_for_version", {
          version,
          dev: mod.dev,
          name: mod.name,
        });
        const list = (Array.isArray(files) ? files : [])
          .map((p) => String(p))
          .filter((p) => p.toLowerCase().endsWith(".cfg"));
        configPath = list[0] ?? "";
        setModCfgFilesByKey((prev) => ({
          ...prev,
          [`${version}::${keyLower}`]: list,
        }));
      } catch {
        configPath = "";
      }
    }

    setModContextMenu({
      open: true,
      x: event.clientX,
      y: event.clientY,
      mod,
      configPath,
    });
  }

  function openVersionContextMenu(event, version) {
    event.preventDefault();
    event.stopPropagation();
    if (!isInstalled(version)) return;
    setVersionContextMenu({
      open: true,
      x: event.clientX,
      y: event.clientY,
      version,
    });
  }

  function startPanelResize(event) {
    if (event.button !== 0) return;
    const container = splitContainerRef.current;
    if (!container) return;

    event.preventDefault();
    setIsResizingPanels(true);

    const handleMove = (moveEvent) => {
      const rect = container.getBoundingClientRect();
      const minLeft = MIN_MOD_PANEL_WIDTH;
      const maxLeft = rect.width - MIN_CONFIG_PANEL_WIDTH;
      if (maxLeft <= minLeft) return;

      const nextLeft = clamp(moveEvent.clientX - rect.left, minLeft, maxLeft);
      const nextPercent = (nextLeft / rect.width) * 100;
      setModPanelWidthPercent(clamp(nextPercent, 30, 70));
    };

    const handleUp = () => {
      setIsResizingPanels(false);
      window.removeEventListener("pointermove", handleMove);
      window.removeEventListener("pointerup", handleUp);
    };

    window.addEventListener("pointermove", handleMove);
    window.addEventListener("pointerup", handleUp);
  }

  async function openSelectedModFolder(mod) {
    const version = Number(selectedVersion);
    if (!Number.isFinite(version) || !mod) return;
    try {
      await invoke("open_mod_folder", {
        version,
        dev: mod.dev,
        name: mod.name,
      });
    } catch (e) {
      window.alert(e?.message ?? String(e));
    } finally {
      closeModContextMenu();
    }
  }

  async function openSelectedConfigFile(configPath) {
    const version = Number(selectedVersion);
    if (!Number.isFinite(version) || !configPath) return;
    try {
      await invoke("open_config_file_for_version", {
        version,
        relPath: configPath,
      });
    } catch (e) {
      window.alert(e?.message ?? String(e));
    } finally {
      closeModContextMenu();
    }
  }

  const updateIsWorking = updatePrompt.open && task.status === "working";
  const updateIsDone = updatePrompt.open && task.status === "done";
  const updateIsError = updatePrompt.open && task.status === "error";

  const RUN_OPTIONS = useMemo(
    () => [
      {
        value: "hq",
        label: "HQ Run",
        buttonLabel: "HQ Run",
        preset: "hq",
        practice: false,
        title: "Normal run (HQ): practice mods are disabled",
      },
      {
        value: "smhq",
        label: "SMHQ Run",
        preset: "smhq",
        practice: false,
        title: "SMHQ preset run",
      },
      {
        value: "practice",
        label: "Normal Practice",
        buttonLabel: "Normal Practice",
        preset: "hq",
        practice: true,
        title: "Practice run: installs/enables practice mods for this run",
      },
      {
        type: "separator",
        key: "run-group-brutal",
      },
      {
        value: "brutal",
        label: "Brutal Run",
        preset: "brutal",
        practice: false,
        title: "Brutal preset: installs Brutal-tagged mods (v49+)",
      },
      {
        value: "brutal_practice",
        label: "Brutal Practice",
        preset: "brutal",
        practice: true,
        title:
          "Brutal preset: installs Brutal-tagged mods + practice mods (v49+)",
      },
      {
        type: "separator",
        key: "run-group-wesley",
      },
      {
        value: "wesley",
        label: "Wesley's Run",
        preset: "wesley",
        practice: false,
        title: "Wesley preset: installs Wesley-tagged mods (v69+)",
      },
      {
        value: "wesley_smhq",
        label: "Wesley's SMHQ",
        preset: "wesley_smhq",
        practice: false,
        title:
          "Wesley + SMHQ preset: installs Wesley-tagged and SMHQ-tagged mods (v69+)",
      },
      {
        value: "wesley_practice",
        label: "Wesley's Practice",
        preset: "wesley",
        practice: true,
        title:
          "Wesley preset: installs Wesley-tagged mods + practice mods (v69+)",
      },
      {
        type: "separator",
        key: "run-group-cmoons",
      },
      {
        value: "c_moons",
        label: "C.Moons Run",
        preset: "c_moons",
        practice: false,
        title: "C.Moons preset: installs C.Moons-tagged mods",
      },
      {
        value: "c_moons_smhq",
        label: "C.Moons SMHQ",
        preset: "c_moons_smhq",
        practice: false,
        title: "C.Moons + SMHQ preset: installs C.Moons-tagged and SMHQ-tagged mods",
      },
      {
        value: "c_moons_practice",
        label: "C.Moons Practice",
        preset: "c_moons",
        practice: true,
        title: "C.Moons preset: installs C.Moons-tagged mods + practice mods",
      },
    ],
    [],
  );

  const selectedRunOption = useMemo(() => {
    return (
      RUN_OPTIONS.find((o) => o.value === runMode) ??
      RUN_OPTIONS.find((o) => o.value === "hq") ??
      RUN_OPTIONS[0]
    );
  }, [RUN_OPTIONS, runMode]);

  const discordRunLabel = useMemo(() => {
    const value = selectedRunOption?.value;
    if (!value) return "High Quota Run";
    if (value === "hq") return "High Quota Run";
    if (value === "practice") return "High Quota Practice";
    if (value === "c_moons") return "Classic Moons Run";
    if (value === "c_moons_smhq") return "Classic Moons SMHQ";
    if (value === "c_moons_practice") return "Classic Moons Practice";
    return selectedRunOption.label;
  }, [selectedRunOption]);

  const discordSmallImage = useMemo(() => {
    if (!selectedRunOption?.value) return null;
    if (selectedRunOption.value.startsWith("brutal")) return "brutal";
    if (selectedRunOption.value.startsWith("wesley")) return "wesleys";
    if (selectedRunOption.value.startsWith("c_moons")) return "cmoons";
    return null;
  }, [selectedRunOption]);

  const discordSmallText = useMemo(() => {
    const value = selectedRunOption?.value;
    if (!value) return discordRunLabel;
    if (value === "c_moons") return "Classic Moons Run";
    if (value === "c_moons_smhq") return "Classic Moons SMHQ";
    if (value === "c_moons_practice") return "Classic Moons Practice";
    return discordRunLabel;
  }, [discordRunLabel, selectedRunOption]);

  const discordPresence = useMemo(() => {
    if (gameStatus.running) {
      return {
        details: `Playing ${discordRunLabel}`,
        large_image: "orange",
        large_text: `${selectedRunOption.value.indexOf("practice") == -1? "grinding": "practicing"} v${selectedVersion}`,
        small_image: discordSmallImage,
        small_text: discordSmallText,
        button_label: "Download",
        button_url: DISCORD_DOWNLOAD_URL,
        use_stream_overlays: true,
      };
    }

    return {
      details: "Idle",
      large_image: "black",
      large_text: "HQ Launcher",
      small_image: discordSmallImage,
      small_text: discordSmallText,
      // state: selectedVersion? `v${selectedVersion}`: "",
      button_label: "Download",
      button_url: DISCORD_DOWNLOAD_URL,
      use_stream_overlays: false,
    };
  }, [
    discordSmallImage,
    discordSmallText,
    discordRunLabel,
    gameStatus.running,
    selectedVersion,
  ]);

  useEffect(() => {
    invoke("set_discord_presence", { payload: discordPresence }).catch(() => {});
  }, [discordPresence]);

  useEffect(() => {
    if (!gameStatus.running) return undefined;

    const intervalId = window.setInterval(() => {
      invoke("set_discord_presence", { payload: discordPresence }).catch(() => {});
    }, 15000);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [discordPresence, gameStatus.running]);

  useEffect(() => {
    return () => {
      invoke("clear_discord_presence").catch(() => {});
    };
  }, []);

  const latestPrepareKeyRef = useRef("");
  const preparePrevRef = useRef(null); // { key, prevRunMode, prevVersion }
  const explicitCancelKeyRef = useRef(""); // only set when user clicks Cancel
  const didAutoPrepareInitialRef = useRef(false);
  const lastPracticeInstallProbeRef = useRef("");

  async function prepareRunMode(nextRunMode, nextVersion, opts = {}) {
    if (gameStatus.running) return;
    if (!opts?.assumeInstalled && !isInstalled(nextVersion)) return;
    const opt =
      RUN_OPTIONS.find((o) => o.value === nextRunMode) ??
      RUN_OPTIONS.find((o) => o.value === "hq") ??
      RUN_OPTIONS[0];

    const key = `${nextRunMode}:${nextVersion}`;
    latestPrepareKeyRef.current = key;
    if (
      typeof opts?.prevRunMode === "string" ||
      typeof opts?.prevVersion === "number"
    ) {
      preparePrevRef.current = {
        key,
        prevRunMode:
          typeof opts?.prevRunMode === "string" ? opts.prevRunMode : runMode,
        prevVersion:
          typeof opts?.prevVersion === "number" ? opts.prevVersion : selectedVersion,
      };
    }

    // Reset modal state; the modals open only if backend emits progress.
    setPresetPrompt({ open: false });
    setPresetTask(null);
    setPresetCancelBusy(false);
    setPracticePrompt({ open: false });
    setPracticeTask(null);
    setPracticeCancelBusy(false);

    try {
      await invoke("prepare_preset", {
        version: nextVersion,
        preset: opt?.preset ?? "hq",
        practice: !!opt?.practice,
      });

      // Ignore stale completions.
      if (latestPrepareKeyRef.current !== key) return;

      const expectedPracticeKeys = !!opt?.practice
        ? (Array.isArray(practiceMods) ? practiceMods : [])
            .filter(
              (m) =>
                !isUiHiddenMod(m) &&
                isModCompatibleWithVersion(m, nextVersion) &&
                !disabledSet.has(modKeyLower(m))
            )
            .map((m) => modKeyLower(m))
        : [];

      await refreshInstalledModVersions(nextVersion, {
        retries: opt?.practice ? 8 : 0,
        delayMs: 300,
        expectedKeys: expectedPracticeKeys,
      });
      await refreshConfigLinkState(nextVersion);

      invoke("get_disabled_mods")
        .then((dm) => setDisabledMods(Array.isArray(dm) ? dm : []))
        .catch(() => {});
      setPreparedUpdateContext(key);
    } catch (e) {
      console.error(e);
      // Ignore stale errors (e.g., cancelled because user picked another mode/version).
      if (latestPrepareKeyRef.current !== key) return;

      const msg = e?.message ?? String(e);
      if (String(msg).toLowerCase().includes("cancelled")) {
        // If user explicitly cancelled this prepare, revert the select menus.
        if (explicitCancelKeyRef.current === key) {
          explicitCancelKeyRef.current = "";
          const prev = preparePrevRef.current;
          if (prev && prev.key === key) {
            // Revert run category + version together (whichever changed).
            setRunMode(prev.prevRunMode);
            setSelectedVersion(prev.prevVersion);
            if (isInstalled(prev.prevVersion)) {
              invoke("apply_disabled_mods", { version: prev.prevVersion }).catch(() => {});
            }
          }
        }
        setPresetPrompt({ open: false });
        setPracticePrompt({ open: false });
        setPresetCancelBusy(false);
        setPracticeCancelBusy(false);
        return;
      }

      if (opt?.practice) {
        setPracticePrompt({ open: true });
        setPracticeTask((t) => ({
          ...(t ?? {}),
          status: "error",
          version: nextVersion,
          step_name: t?.step_name ?? "Practice Mods",
          detail: t?.detail ?? "Failed to prepare practice mods",
          error: msg,
        }));
      }
      setPresetPrompt({ open: true });
      setPresetTask((t) => ({
        ...(t ?? {}),
        status: "error",
        version: nextVersion,
        step_name: t?.step_name ?? "Preset Mods",
        detail: t?.detail ?? "Failed to prepare preset mods",
        error: msg,
      }));
      setTask((t) => ({
        ...t,
        status: "error",
        version: nextVersion,
        error: msg,
      }));
    }
  }

  async function stopRun() {
    if (!gameStatus.running) return;
    try {
      await invoke("stop_game");
    } finally {
      setGameStatus({ running: false, pid: null });
    }
  }

  async function saveSteamOverlaySettings() {
    setSteamOverlaySaveBusy(true);
    setSteamOverlayError("");
    setSteamOverlaySaved("");
    try {
      const saved = await invoke("set_steam_overlay_config", {
        enabled: !!steamOverlayConfig.enabled,
        steamPath: steamOverlayConfig.steam_path.trim() || null,
      });
      setSteamOverlayResolvedPath(String(saved?.resolved_steam_path ?? ""));
      setSteamOverlayConfig({
        enabled: !!saved?.enabled,
        steam_path: String(saved?.steam_path ?? saved?.resolved_steam_path ?? ""),
      });
      setSteamOverlaySaved("Saved");
      setSteamOverlayDialogOpen(false);
    } catch (error) {
      setSteamOverlayError(error?.message ?? String(error));
    } finally {
      setSteamOverlaySaveBusy(false);
    }
  }

  async function browseSteamOverlayPath() {
    if (steamOverlaySaveBusy) return;
    setSteamOverlayError("");
    setSteamOverlaySaved("");
    try {
      const picked = await invoke("pick_steam_overlay_path", {
        initialPath: steamOverlayConfig.steam_path || steamOverlayResolvedPath || null,
      });
      if (!picked) return;
      setSteamOverlayConfig((prev) => ({
        ...prev,
        steam_path: String(picked),
      }));
    } catch (error) {
      setSteamOverlayError(error?.message ?? String(error));
    }
  }

  async function startSelectedRun(opts = {}) {
    if (launchBusy) return;
    if (gameStatus.running) return stopRun();

    const nextRunMode =
      typeof opts?.runMode === "string" && opts.runMode
        ? opts.runMode
        : runModeRef.current;
    const nextVersion = Number(
      opts?.version ?? selectedVersionRef.current ?? selectedVersion
    );

    if (!Number.isFinite(nextVersion)) return;

    if (!isInstalled(nextVersion)) {
      openDownloadPrompt(nextVersion);
      return;
    }

    setLaunchBusy(true);
    try {
      const launchRequest = getLaunchRequestForRunMode(nextRunMode, nextVersion);
      const pid = await invoke(launchRequest.command, launchRequest.args);
      setGameStatus({
        running: true,
        pid: typeof pid === "number" ? pid : null,
      });
    } catch (e) {
      console.error(e);
      setTask((t) => ({
        ...t,
        status: "error",
        version: nextVersion,
        error: e?.message ?? String(e),
      }));
    } finally {
      setLaunchBusy(false);
    }
  }

  useEffect(() => {
    if (!didFinishBootstrap) return;
    if (didAutoPrepareInitialRef.current) return;
    if (gameStatus.running) return;

    const version = Number(selectedVersion);
    if (!Number.isFinite(version)) return;
    if (!isVersionWithinRange(version, selectedRunModeRange)) {
      return;
    }
    if (!isInstalled(version)) return;

    didAutoPrepareInitialRef.current = true;
    prepareRunMode(runMode, version).catch((e) => {
      console.error(e);
      didAutoPrepareInitialRef.current = false;
    });
  }, [
    didFinishBootstrap,
    gameStatus.running,
    isInstalled,
    selectedRunModeRange,
    runMode,
    selectedVersion,
  ]);

  useEffect(() => {
    if (!didFinishBootstrap) return;
    if (!isPracticeRunMode(runMode)) {
      lastPracticeInstallProbeRef.current = "";
      return;
    }
    const version = Number(selectedVersion);
    if (!Number.isFinite(version)) return;
    if (!isInstalled(version)) return;

    const missingKeys = practiceReferenceMods
      .filter((m) => !disabledSet.has(modKeyLower(m)))
      .map((m) => modKeyLower(m))
      .filter((key) => !installedModVersions[key]);

    if (missingKeys.length === 0) {
      lastPracticeInstallProbeRef.current = "";
      return;
    }

    const probeKey = `${version}:${missingKeys.slice().sort().join("|")}`;
    if (lastPracticeInstallProbeRef.current === probeKey) return;
    lastPracticeInstallProbeRef.current = probeKey;

    refreshInstalledModVersions(version, {
      retries: 8,
      delayMs: 400,
      expectedKeys: missingKeys,
    }).finally(() => {
      if (lastPracticeInstallProbeRef.current === probeKey) {
        lastPracticeInstallProbeRef.current = "";
      }
    });
  }, [
    didFinishBootstrap,
    disabledSet,
    installedModVersions,
    isInstalled,
    practiceReferenceMods,
    runMode,
    selectedVersion,
  ]);

  // Save selectedVersion to localStorage
  useEffect(() => {
    if (selectedVersion != null) {
      localStorage.setItem("selectedVersion", String(selectedVersion));
    }
  }, [selectedVersion]);

  useEffect(() => {
    if (runMode) {
      localStorage.setItem("selectedRunMode", runMode);
    }
  }, [runMode]);

  async function handleRunModeSelect(nextRunMode) {
    const prevRun = runMode;
    const prevVer = selectedVersion;
    const range = getPresetVersionRange(manifest, nextRunMode);
    const effectiveV = clampVersionToRange(selectedVersion, range);

    setRunMode(nextRunMode);
    if (effectiveV !== selectedVersion) {
      setSelectedVersion(effectiveV);
      if (isInstalled(effectiveV)) {
        try {
          await invoke("apply_disabled_mods", { version: effectiveV });
        } catch {}
      } else {
        openDownloadPrompt(effectiveV);
        return;
      }
    }
    await prepareRunMode(nextRunMode, effectiveV, {
      prevRunMode: prevRun,
      prevVersion: prevVer,
    });
  }

  const showBootstrapSkeleton = !didFinishBootstrap;
  const hasSelectedVersionUpdates =
    checkUpdateTask.status === "done" &&
    Number(checkUpdateTask.version) === Number(selectedVersion) &&
    checkUpdateTask.run_mode === runMode &&
    (checkUpdateTask.updatable_mods?.length ?? 0) > 0;

  return (
    <div className="h-full text-white">
      <div className="mx-auto flex h-full max-w-[1600px] flex-col gap-4 p-4">
        {showBootstrapSkeleton ? (
          <LauncherPageSkeleton statusText={bootstrapStatus} />
        ) : (
          <>
        {/* Top bar */}
        <div className="flex items-center gap-3">
          {gameStatus.running ? (
            <Button
              variant="secondary"
              className="h-11 px-5"
              onClick={stopRun}
              title="Stop"
            >
              <Play className="h-4 w-4" />
              Stop
            </Button>
          ) : (
            <div
              className="flex h-11 overflow-hidden rounded-xl border border-black/10 bg-white text-black shadow-sm"
              title={selectedRunOption?.title ?? ""}
            >
              <button
                type="button"
                className="flex h-full select-none items-center gap-2 px-5 text-[14px] font-[620] tracking-[-0.014em] transition-colors hover:bg-black/[0.04] disabled:cursor-not-allowed disabled:opacity-70"
                disabled={launchBusy}
                onClick={() =>
                  startSelectedRun({
                    runMode,
                    version: selectedVersion,
                  })
                }
              >
                {launchBusy ? (
                  <LoaderCircle className="h-4 w-4 animate-spin" />
                ) : (
                  <Play className="h-4 w-4" />
                )}
                {selectedRunOption?.buttonLabel ??
                  selectedRunOption?.label ??
                  "Start Run"}
              </button>

              <DropdownMenu.Root modal={false}>
                <DropdownMenu.Trigger asChild>
                  <button
                    type="button"
                    disabled={launchBusy}
                    className="flex h-full w-10 shrink-0 items-center justify-center rounded-none border-0 border-l border-black/10 bg-transparent px-0 text-black transition-colors hover:bg-black/[0.04] focus:outline-none focus:ring-0 disabled:cursor-not-allowed disabled:opacity-70"
                    aria-label="Select run mode"
                  >
                    <ChevronDown className="h-4 w-4 text-black/70" />
                    <span className="sr-only">Select run mode</span>
                  </button>
                </DropdownMenu.Trigger>
                <DropdownMenu.Portal>
                  <ScrollableDropdownContent
                    sideOffset={8}
                    align="start"
                    className="z-50 min-w-48 rounded-[18px] border border-white/10 bg-[#12141a] p-1 shadow-2xl shadow-black/45"
                    scrollAreaClassName="max-h-64"
                  >
                    {RUN_OPTIONS.map((opt) =>
                      opt.type === "separator" ? (
                        <DropdownMenu.Separator
                          key={opt.key}
                          className="mx-2 my-1 h-px bg-white/20"
                        />
                      ) : (
                        <DropdownMenu.Item
                          key={opt.value}
                          onSelect={() => {
                            handleRunModeSelect(opt.value).catch(console.error);
                          }}
                          className={cn(
                            "flex cursor-pointer select-none items-center gap-2 rounded-xl px-3 py-2 text-[14px] font-medium tracking-[-0.012em] text-white/85 outline-none transition focus:bg-white/10",
                            runMode === opt.value ? "bg-white/10" : "",
                          )}
                        >
                          <span className="inline-flex w-5 items-center justify-center">
                            {runMode === opt.value ? (
                              <Check className="h-4 w-4 text-emerald-300" />
                            ) : null}
                          </span>
                          <span className="min-w-0 flex-1">{opt.label}</span>
                        </DropdownMenu.Item>
                      ),
                    )}
                  </ScrollableDropdownContent>
                </DropdownMenu.Portal>
              </DropdownMenu.Root>
            </div>
          )}

          <div className="w-fit">
            <DropdownMenu.Root modal={false}>
              <DropdownMenu.Trigger asChild>
                <button
                  type="button"
                  className="flex h-11 items-center gap-1 rounded-xl border border-panel-outline bg-white/5 pl-3 pr-2.5 text-[14px] font-medium tracking-[-0.012em] text-white outline-none transition-colors duration-150 hover:bg-white/10 focus:ring-2 focus:ring-panel-outline data-[state=open]:bg-white/10"
                  onPointerDown={(e) => {
                    if (e.button !== 2) return;
                    e.preventDefault();
                    e.stopPropagation();
                  }}
                  onContextMenu={(e) => {
                    if (!selectedInstalled) return;
                    openVersionContextMenu(e, Number(selectedVersion));
                  }}
                >
                  <div className="flex items-center gap-2">
                    <div className="text-[14px] font-[620] tracking-[-0.014em]">
                      {selectedVersionLabel}
                    </div>
                    {selectedInstalled ? (
                      <CheckCircle2 className="h-4 w-4 text-emerald-400" />
                    ) : (
                      <Download className="h-4 w-4 text-amber-300" />
                    )}
                  </div>
                  <ChevronDown className="h-4 w-4 text-white/45" />
                </button>
              </DropdownMenu.Trigger>

              <DropdownMenu.Portal>
                <ScrollableDropdownContent
                  sideOffset={8}
                  align="start"
                  className="z-50 min-w-[170px] rounded-[18px] border border-panel-outline bg-[#12141a] p-1 shadow-2xl shadow-black/45"
                  scrollAreaClassName="max-h-[min(20rem,calc(100vh-8rem))]"
                >
                  {versionOptions.map((v) => {
                    const installed = isInstalled(v);
                    const active = Number(selectedVersion) === Number(v);
                    return (
                      <DropdownMenu.Item
                        key={v}
                        onSelect={async () => {
                          const nextV = Number(v);
                          if (installed) {
                            const prevRun = runMode;
                            const prevVer = selectedVersion;
                            setSelectedVersion(nextV);
                            try {
                              await invoke("apply_disabled_mods", { version: nextV });
                            } catch {}
                            await prepareRunMode(runMode, nextV, {
                              prevRunMode: prevRun,
                              prevVersion: prevVer,
                            });
                          } else {
                            openDownloadPrompt(nextV);
                          }
                        }}
                        onContextMenu={(e) => {
                          openVersionContextMenu(e, v);
                        }}
                        className={cn(
                          "flex cursor-pointer select-none items-center justify-between gap-3 rounded-xl px-3 py-2 text-[14px] font-medium tracking-[-0.012em] text-white/85 outline-none transition focus:bg-white/10",
                          active ? "bg-white/10" : "",
                        )}
                      >
                        <span className="inline-flex min-w-0 items-center gap-2.5">
                          {active ? (
                            <Check className="h-4 w-4 shrink-0 text-emerald-300" />
                          ) : !installed ? (
                            <Download className="h-4 w-4 shrink-0 text-amber-300" />
                          ) : (
                            <span className="h-4 w-4 shrink-0" />
                          )}
                          <span
                            className={cn(
                              active ? "font-[620] text-white" : "font-medium text-white/90"
                            )}
                          >
                            v{v}
                          </span>
                        </span>
                        <span className="shrink-0 text-[13px] font-medium tracking-[-0.01em] text-white/38">
                          ({installed ? "installed" : "download"})
                        </span>
                      </DropdownMenu.Item>
                    );
                  })}
                </ScrollableDropdownContent>
              </DropdownMenu.Portal>
            </DropdownMenu.Root>
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
              placeholder="Search mods"
              className="h-11 pl-10"
            />
          </div>

          {hasSelectedVersionUpdates && <Button
            variant="secondary"
            className="h-11"
            onClick={() => {
              const sameContext =
                Number(checkUpdateTask.version) === Number(selectedVersion) &&
                checkUpdateTask.run_mode === runMode;
              const alreadyChecked =
                sameContext && checkUpdateTask.status === "done";
              const isChecking =
                sameContext && checkUpdateTask.status === "working";

              // Don't re-check if we already have results for this version.
              // If it's currently checking, just open the modal to show progress.
              if (!alreadyChecked && !isChecking) {
                checkModUpdates(selectedVersion, { runMode });
              }
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
                  Logged in
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
        <div
          ref={showResizablePanels ? splitContainerRef : null}
          className={cn(
            "min-h-0 flex-1",
            showResizablePanels
              ? "flex gap-0 overflow-hidden"
              : selectedMod
              ? "grid grid-cols-1 gap-4"
              : "grid grid-cols-1"
          )}
        >
          {/* Mod list */}
          <div
            className={cn(
              "min-h-0 rounded-2xl border border-panel-outline bg-white/5 p-3",
              showResizablePanels ? "shrink-0 rounded-r-none border-r-0" : ""
            )}
            style={
              showResizablePanels
                ? {
                    width: `${modPanelWidthPercent}%`,
                    minWidth: `${MIN_MOD_PANEL_WIDTH}px`,
                    maxWidth: `calc(100% - ${MIN_CONFIG_PANEL_WIDTH}px)`,
                  }
                : undefined
            }
          >
            <div className="mb-3 flex items-center justify-between px-1">
              <div className="flex items-center gap-2">
                <div className="text-sm font-semibold text-white/80">
                  Mods
                </div>
                <button
                  type="button"
                  className="inline-flex items-center gap-1 rounded-lg border border-panel-outline bg-white/5 px-2 py-1 text-[11px] font-medium text-white/65 transition hover:bg-white/10 hover:text-white"
                  onClick={() => {
                    setSteamOverlayDialogOpen(true);
                    setSteamOverlayError("");
                    setSteamOverlaySaved("");
                  }}
                  title="Inject Steam Overlay settings"
                >
                  <Settings2 className="h-3.5 w-3.5" />
                  Overlay
                </button>
              </div>
              <div className="text-xs text-white/40">
                {displayedMods.length} items
              </div>
            </div>

            <div className="h-[calc(100%-2.25rem)] overflow-auto pr-1">
              <div className="flex flex-col gap-2">
                {displayedMods.map((m) => {
                  const presetSummary = isPresetSummaryMod(m);
                  const selected =
                    selectedMod && listEntryKey(selectedMod) === listEntryKey(m);
                  const initials = `${m.dev?.[0] ?? "M"}${
                    m.name?.[0] ?? "M"
                  }`.toUpperCase();
                  const keyLower = modKeyLower(m);
                  const coverSrc = presetSummary ? m.iconSrc : installedModIconUrls[keyLower];
                  const description = presetSummary
                    ? m.description
                    : installedModDescriptions[keyLower] || "Click to edit config";
                  const smhqEnableLocked =
                    isSmhqRunMode(runMode) && smhqForcedModKeys.has(keyLower);
                  const enabled =
                    smhqEnableLocked ||
                    (!disabledSet.has(keyLower) && !practiceLockedModKeys.has(keyLower));
                  const installedVer = installedModVersions[keyLower];
                  const busy = modToggleBusyKeys.has(keyLower);
                  const isPracticeMod =
                    isPracticeRunMode(runMode) && practiceModKeys.has(keyLower);
                  const practiceEnableLocked =
                    isPracticeRunMode(runMode) && practiceLockedModKeys.has(keyLower);
                  return (
                    <div
                      key={listEntryKey(m)}
                      className={cn(
                        "group flex w-full items-center gap-3 rounded-2xl border px-3 py-3 text-left transition",
                        selected
                          ? "border-panel-outline bg-white/10"
                          : "border-panel-outline bg-black/10 hover:bg-white/10",
                        !presetSummary && !installedVer && "opacity-40"
                      )}
                      onClick={() => setSelectedMod(m)}
                      onContextMenu={(e) => {
                        if (presetSummary) return;
                        openModContextMenu(e, m);
                      }}
                      role="button"
                      tabIndex={0}
                      onKeyDown={(e) => {
                        if (e.key === "Enter" || e.key === " ") {
                          e.preventDefault();
                          setSelectedMod(m);
                        }
                      }}
                    >
                      <ModCover src={coverSrc} initials={initials} />
                      <div className="min-w-0 flex-1">
                        <div className="flex items-baseline gap-2">
                          <div className="truncate text-base font-semibold">
                            {m.name}
                          </div>
                          <div className="truncate text-sm text-white/40">
                            {m.dev}
                          </div>
                          {/* {presetSummary ? (
                            <div className="rounded-full border border-sky-400/30 bg-sky-400/10 px-2 py-0.5 text-[11px] font-medium text-sky-100">
                              Locked
                            </div>
                          ) : null} */}
                          {isPracticeMod ? (
                            <div className="rounded-full border border-emerald-400/30 bg-emerald-400/10 px-2 py-0.5 text-[11px] font-medium text-emerald-200">
                              Practice
                            </div>
                          ) : null}
                          {smhqEnableLocked ? (
                            <div className="rounded-full border border-sky-400/30 bg-sky-400/10 px-2 py-0.5 text-[11px] font-medium text-sky-200">
                              SMHQ
                            </div>
                          ) : null}
                        </div>
                        <div
                          className="mt-1 overflow-hidden whitespace-nowrap text-sm text-white/50"
                          title={description}
                          style={{
                            maskImage:
                              "linear-gradient(90deg, black 0%, black 82%, transparent 100%)",
                            WebkitMaskImage:
                              "linear-gradient(90deg, black 0%, black 82%, transparent 100%)",
                          }}
                        >
                          {description}
                        </div>
                      </div>
                      <div className="self-stretch flex shrink-0 items-center">
                        {presetSummary ? (
                          <div className="rounded-full border border-white/10 bg-white/5 px-2.5 py-1 text-[11px] font-medium text-white/65">
                            {m.totalCount} mods
                          </div>
                        ) : (
                          <div
                            onClick={(e) => e.stopPropagation()}
                            onKeyDown={(e) => e.stopPropagation()}
                            className="inline-flex"
                          >
                            <Switch
                              checked={enabled}
                              disabled={busy || practiceEnableLocked || smhqEnableLocked}
                              onCheckedChange={(v) =>
                                toggleModEnabledForMod(m, !!v)
                              }
                            />
                          </div>
                        )}
                      </div>
                      {/* <Settings2 className="mt-1 h-4 w-4 shrink-0 text-white/30 opacity-0 transition group-hover:opacity-100" /> */}
                    </div>
                  );
                })}
                {displayedMods.length === 0 && (
                  <div className="px-2 py-10 text-center text-sm text-white/40">
                    No mods found.
                  </div>
                )}
              </div>
            </div>
          </div>

          {/* Right panel: config editor */}
          {!selectedMod ? null : (
            <>
            {showResizablePanels ? (
              <div className="relative z-10 flex w-0 shrink-0 items-stretch justify-center">
                <button
                  type="button"
                  aria-label="Resize panels"
                  className="group absolute inset-y-0 left-1/2 w-6 -translate-x-1/2 cursor-col-resize"
                  onPointerDown={startPanelResize}
                >
                  <span
                    className={cn(
                      "pointer-events-none absolute inset-y-0 left-1/2 w-px -translate-x-1/2 bg-[#2C313A] transition-colors",
                      isResizingPanels
                        ? "bg-[#00C896]"
                        : "group-hover:bg-[#434B58]"
                    )}
                  />
                  <span
                    className={cn(
                      "pointer-events-none absolute left-1/2 top-1/2 h-14 w-1.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-[#3A404A] transition-colors",
                      isResizingPanels
                        ? "bg-[#00C896]"
                        : "group-hover:bg-[#596273]"
                    )}
                  />
                </button>
              </div>
            ) : null}
            <div
              className={cn(
                "min-h-0 rounded-2xl border border-panel-outline bg-white/5 p-4",
                showResizablePanels ? "min-w-0 flex-1 rounded-l-none border-l-0" : ""
              )}
            >
              <div className="flex h-full flex-col gap-3">
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <div className="flex items-baseline gap-2">
                      <div className="truncate text-lg font-semibold">
                        {selectedMod.name}
                      </div>
                      <div className="truncate text-sm text-white/40">
                        {selectedMod.dev}
                      </div>
                    </div>
                    {selectedPresetSummary ? (
                      <>
                        <div className="mt-1 line-clamp-2 text-sm text-white/55">
                          {selectedPresetSummary.installedCount} / {selectedPresetSummary.totalCount} preset mods are installed for v{selectedVersion}.
                        </div>
                      </>
                    ) : null}
                    {selectedPresetSummary ? null : (() => {
                      const k = `${String(selectedMod.dev).toLowerCase()}::${String(
                        selectedMod.name
                      ).toLowerCase()}`;
                      const description = installedModDescriptions[k];
                      if (!description) return null;
                      return (
                        <div className="mt-1 line-clamp-2 text-sm text-white/55">
                          {description}
                        </div>
                      );
                    })()}
                    {selectedPresetSummary ? null : (
                    <div className="mt-1 text-xs text-white/45">
                      {(() => {
                        const k = `${String(selectedMod.dev).toLowerCase()}::${String(
                          selectedMod.name
                        ).toLowerCase()}`;
                        const v = installedModVersions[k];
                        const forcedOff = practiceLockedModKeys.has(k);
                        const userOff = disabledSet.has(k);
                        const stateLabel = forcedOff
                          ? "Off in Practice"
                          : userOff
                          ? "Disabled"
                          : "Enabled";
                        return `${v ? `Installed: v${v}` : "Not installed"} · ${stateLabel}`;
                      })()}
                    </div>
                    )}
                  </div>
                  <button
                    type="button"
                    aria-label="Close config editor"
                    className="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-transparent text-[#8E97A5] transition-colors hover:bg-[#242830] hover:text-white focus:outline-none focus:ring-2 focus:ring-panel-outline"
                    onClick={() => {
                      setSelectedMod(null);
                    }}
                  >
                    <X className="h-4 w-4" />
                  </button>
                </div>

                {selectedPresetSummary ? (
                  <>
                    {/* <div className="grid grid-cols-1 gap-2 md:grid-cols-3">
                      <div className="rounded-2xl border border-panel-outline bg-black/10 px-3 py-2">
                        <div className="text-[11px] font-semibold uppercase tracking-[0.08em] text-white/40">
                          Preset Tags
                        </div>
                        <div className="mt-1 text-sm text-white/85">
                          {selectedPresetSummary.summaryTags.join(" + ")}
                        </div>
                      </div>
                      <div className="rounded-2xl border border-panel-outline bg-black/10 px-3 py-2">
                        <div className="text-[11px] font-semibold uppercase tracking-[0.08em] text-white/40">
                          Installed
                        </div>
                        <div className="mt-1 text-sm text-white/85">
                          {selectedPresetSummary.installedCount} / {selectedPresetSummary.totalCount}
                        </div>
                      </div>
                      <div className="rounded-2xl border border-panel-outline bg-black/10 px-3 py-2">
                        <div className="text-[11px] font-semibold uppercase tracking-[0.08em] text-white/40">
                          State
                        </div>
                        <div className="mt-1 text-sm text-white/85">
                          Always enabled by preset
                        </div>
                      </div>
                    </div> */}

                    <div className="min-h-0 flex flex-1 overflow-hidden">
                      <div className="min-h-0 flex-1 overflow-auto rounded-2xl border border-panel-outline bg-black/10 p-3">
                        {selectedPresetSummary.summaryItems.length === 0 ? (
                          <div className="flex h-full items-center justify-center text-sm text-white/40">
                            No preset-tagged mods are available for this version.
                          </div>
                        ) : (
                          <div className="flex flex-col gap-2">
                            {selectedPresetSummary.summaryItems.map((mod) => {
                              const keyLower = modKeyLower(mod);
                              const installedVer = installedModVersions[keyLower];
                              const iconSrc = installedModIconUrls[keyLower];
                              const description =
                                installedModDescriptions[keyLower] ||
                                "Preset-tagged mod";
                              const disabledByUser = disabledSet.has(keyLower);
                              return (
                                <div
                                  key={modKey(mod)}
                                  className="flex items-center gap-3 rounded-2xl border border-panel-outline bg-white/5 px-3 py-3"
                                  onContextMenu={(e) => openModContextMenu(e, mod)}
                                >
                                  <ModCover
                                    src={iconSrc}
                                    initials={`${mod.dev?.[0] ?? "M"}${
                                      mod.name?.[0] ?? "M"
                                    }`.toUpperCase()}
                                  />
                                  <div className="min-w-0 flex-1">
                                    <div className="flex items-baseline gap-2">
                                      <div className="truncate text-sm font-semibold text-white/90">
                                        {mod.name}
                                      </div>
                                      <div className="truncate text-xs text-white/40">
                                        {mod.dev}
                                      </div>
                                    </div>
                                    <div className="mt-1 truncate text-xs text-white/50">
                                      {description}
                                    </div>
                                  </div>
                                  <div className="shrink-0 text-right">
                                    <div className="text-xs font-medium text-white/75">
                                      {installedVer
                                        ? `v${installedVer}`
                                        : selectedInstalled
                                        ? "Missing"
                                        : "Pending"}
                                    </div>
                                    <div className="mt-1 text-[11px] text-white/40">
                                      {disabledByUser
                                        ? "User disabled"
                                        : installedVer
                                        ? "Installed"
                                        : selectedInstalled
                                        ? "Will sync on prepare"
                                        : "Needs download"}
                                    </div>
                                  </div>
                                </div>
                              );
                            })}
                          </div>
                        )}
                      </div>
                    </div>
                  </>
                ) : (
                  <>
                {configLinkState?.is_installed && configLinkState?.is_linked === false && (
                  <div className="rounded-xl border border-amber-400/20 bg-amber-400/10 px-3 py-2 text-xs text-amber-200">
                    Config is currently <span className="font-semibold">unlinked</span>. Changes will be
                    saved to <span className="font-semibold">this version</span> on (v{selectedVersion}).
                  </div>
                )}

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
                    <div className="min-h-0 flex-1 overflow-auto rounded-2xl border border-panel-outline bg-black/10 p-3">
                    
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
                                  className="rounded-2xl border border-panel-outline bg-white/5 p-3"
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
                  </>
                )}
              </div>
            </div>
            </>
          )}
        </div>
          </>
        )}
      </div>

      {modContextMenu.open && modContextMenu.mod && (
        <div
          ref={modContextMenuRef}
          className="fixed z-[70] min-w-[220px] overflow-hidden rounded-2xl border border-panel-outline bg-[#14161c] p-1.5 shadow-2xl shadow-black/40"
          style={{
            left: modContextMenu.x,
            top: modContextMenu.y,
          }}
        >
          <button
            className="flex w-full items-center rounded-xl px-3 py-2 text-left text-sm text-white/85 transition hover:bg-white/10"
            onClick={() => openSelectedModFolder(modContextMenu.mod)}
          >
            Open Mod Folder
          </button>
          {modContextMenu.configPath ? (
            <button
              className="flex w-full items-center rounded-xl px-3 py-2 text-left text-sm text-white/85 transition hover:bg-white/10"
              onClick={() => openSelectedConfigFile(modContextMenu.configPath)}
            >
              Open Config File
            </button>
          ) : null}
        </div>
      )}

      {versionContextMenu.open && Number.isFinite(versionContextMenu.version) && (
        <div
          ref={versionContextMenuRef}
          className="fixed z-[70] min-w-[220px] overflow-hidden rounded-2xl border border-panel-outline bg-[#14161c] p-1.5 shadow-2xl shadow-black/40"
          style={{
            left: versionContextMenu.x,
            top: versionContextMenu.y,
          }}
        >
          <button
            className="flex w-full items-center rounded-xl px-3 py-2 text-left text-sm text-white/85 transition hover:bg-white/10 disabled:pointer-events-none disabled:opacity-40"
            disabled={gameStatus.running || deleteVersionBusy}
            onClick={() => {
              setDeleteVersionPrompt({
                ...makeDeleteVersionPromptState(),
                open: true,
                version: versionContextMenu.version,
              });
              closeVersionContextMenu();
            }}
          >
            Delete v{versionContextMenu.version}
          </button>
        </div>
      )}

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

          <div className="relative w-[min(520px,calc(100vw-2rem))] rounded-2xl border border-panel-outline bg-[#0f1116] p-5 shadow-2xl shadow-black/50">
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
              <div className="mt-4 rounded-2xl border border-panel-outline bg-white/5 p-3">
                <div className="flex items-center justify-between gap-3">
                  <div className="min-w-0">
                    <div className="truncate text-sm font-semibold">
                      {statusText}
                    </div>
                    <div className="truncate text-xs text-white/50">
                      {task.detail ||
                        (bytesText ? `Downloaded: ${bytesText}` : "")}
                    </div>
                    {(formatTransferProgress(task) ||
                      formatExtractProgress(task)) && (
                      <div className="mt-1 text-xs text-white/40">
                        {[
                          formatTransferProgress(task),
                          formatExtractProgress(task),
                        ]
                          .filter(Boolean)
                          .join(" • ")}
                      </div>
                    )}
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

      {/* Delete version confirm modal */}
      {deleteVersionPrompt.open && (
        <div className="fixed inset-0 z-50 flex items-center justify-center">
          <button
            className="absolute inset-0 bg-black/60"
            onClick={() => {
              if (!deleteVersionBusy) {
                setDeleteVersionPrompt(makeDeleteVersionPromptState());
              }
            }}
            aria-label="Close"
          />

          <div className="relative w-[min(520px,calc(100vw-2rem))] rounded-2xl border border-panel-outline bg-[#0f1116] p-5 shadow-2xl shadow-black/50">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <div className="text-lg font-semibold">
                  {deleteVersionBusy
                    ? `Deleting v${deleteVersionPrompt.version}...`
                    : `Delete v${deleteVersionPrompt.version}?`}
                </div>
                <div className="mt-1 text-sm text-white/55">
                  {deleteVersionBusy
                    ? "Removing installed files for this version."
                    : "This removes the installed files for this version from the launcher."}
                </div>
              </div>
            </div>

            {(deleteVersionBusy ||
              deleteVersionPrompt.status === "working" ||
              deleteVersionPrompt.status === "done") && (
              <div className="mt-4 rounded-2xl border border-panel-outline bg-white/5 p-3">
                <div className="flex items-center justify-between gap-3">
                  <div className="min-w-0">
                    <div className="truncate text-sm font-semibold">
                      {deleteVersionPrompt.detail || "Deleting files..."}
                    </div>
                    <div className="truncate text-xs text-white/50">
                      {Number.isFinite(Number(deleteVersionPrompt.total_files)) &&
                      Number(deleteVersionPrompt.total_files) > 0
                        ? `${Number(deleteVersionPrompt.deleted_files ?? 0)} / ${Number(
                            deleteVersionPrompt.total_files ?? 0
                          )} items removed`
                        : "Scanning files..."}
                    </div>
                  </div>
                  <div className="shrink-0 text-sm text-white/70">
                    {Math.round(Number(deleteVersionPrompt.overall_percent ?? 0))}%
                  </div>
                </div>
                <div className="mt-2 h-2 w-full overflow-hidden rounded-full bg-white/10">
                  <div
                    className="h-full rounded-full bg-emerald-400 transition-[width]"
                    style={{
                      width: `${Math.max(
                        0,
                        Math.min(100, Number(deleteVersionPrompt.overall_percent ?? 0))
                      )}%`,
                    }}
                  />
                </div>
              </div>
            )}

            {deleteVersionPrompt.error ? (
              <div className="mt-4 rounded-2xl border border-red-400/30 bg-red-400/10 p-3 text-sm text-red-200">
                {deleteVersionPrompt.error}
              </div>
            ) : null}

            <div className="mt-5 flex items-center justify-end gap-2">
              <Button
                variant="secondary"
                className="h-10 min-w-[120px]"
                disabled={deleteVersionBusy}
                onClick={() => setDeleteVersionPrompt(makeDeleteVersionPromptState())}
              >
                Cancel
              </Button>
              <Button
                variant="default"
                className="h-10 min-w-[120px]"
                disabled={deleteVersionBusy}
                onClick={() => deleteVersion(deleteVersionPrompt.version)}
              >
                {deleteVersionBusy ? (
                  <>
                    <LoaderCircle className="h-4 w-4 animate-spin" />
                    Deleting...
                  </>
                ) : (
                  <>
                    <Trash2 className="h-4 w-4" />
                    Delete
                  </>
                )}
              </Button>
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

          <div className="relative w-[min(520px,calc(100vw-2rem))] rounded-2xl border border-panel-outline bg-[#0f1116] p-5 shadow-2xl shadow-black/50">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <div className="text-lg font-semibold">
                  {checkUpdateTask.status === "done"
                    ? `${checkUpdateTask.updatable_mods.length} mods can be updated`
                    : "Checking mod versions..."}
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

          <div className="relative w-[min(520px,calc(100vw-2rem))] rounded-2xl border border-panel-outline bg-[#0f1116] p-5 shadow-2xl shadow-black/50">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <div className="text-lg font-semibold">
                  {manifestUpdateInfo && !updateIsWorking && !updateIsDone && !updateIsError
                    ? "Update available"
                    : updateIsDone
                    ? "Update complete"
                    : updateIsError
                    ? "Update failed"
                    : "Updating..."}
                </div>
                <div className="mt-1 text-sm text-white/55">
                  {manifestUpdateInfo && !updateIsWorking && !updateIsDone && !updateIsError
                    ? `v${manifestUpdateInfo.version ?? selectedVersion} version's allowed manifest has changed.`
                    : "Based on the remote manifest, installed game and mod files are being synced to the desired state."}
                </div>
              </div>
            </div>

            {manifestUpdateInfo && !updateIsWorking && !updateIsDone && !updateIsError && (
              <div className="mt-4 rounded-2xl border border-panel-outline bg-white/5 p-3 text-sm text-white/70">
                <div>
                  Installed version: v{manifestUpdateInfo.version ?? selectedVersion}
                </div>
                <div>
                  Previous manifest:{" "}
                  {manifestUpdateInfo.local_depot_manifest ?? "unknown"}
                </div>
                <div>
                  Current manifest:{" "}
                  {manifestUpdateInfo.remote_depot_manifest ?? "unknown"}
                </div>
              </div>
            )}

            {(updateIsWorking || updateIsDone || updateIsError) && (
              <div className="mt-4 rounded-2xl border border-panel-outline bg-white/5 p-3">
                <div className="flex items-center justify-between gap-3">
                  <div className="min-w-0">
                    <div className="truncate text-sm font-semibold">
                      {statusText}
                    </div>
                    <div className="truncate text-xs text-white/50">
                      {task.detail || ""}
                    </div>
                    {(formatTransferProgress(task) ||
                      formatExtractProgress(task)) && (
                      <div className="mt-1 text-xs text-white/40">
                        {[
                          formatTransferProgress(task),
                          formatExtractProgress(task),
                        ]
                          .filter(Boolean)
                          .join(" • ")}
                      </div>
                    )}
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
              ) : manifestUpdateInfo && !updateIsDone && !updateIsError ? (
                <>
                  <Button
                    variant="secondary"
                    className="h-10 min-w-[120px]"
                    onClick={() => {
                      setUpdatePrompt({ open: false });
                      setManifestUpdateInfo(null);
                    }}
                  >
                    Later
                  </Button>
                  <Button
                    variant="default"
                    className="h-10 min-w-[120px]"
                    onClick={async () => {
                      const v =
                        Number(manifestUpdateInfo.version ?? selectedVersion) || selectedVersion;
                      setManifestUpdateInfo(null);
                      setTask((t) => ({
                        ...t,
                        status: "working",
                        version: v,
                        overall_percent: 0,
                        error: null,
                      }));
                      try {
                        await invoke("sync_latest_install_from_manifest", {
                          version: v,
                        });
                      } catch (e) {
                        if (
                          typeof onRequireLogin === "function" &&
                          isAuthError(e)
                        ) {
                          try {
                            const didLogin = await onRequireLogin();
                            if (!didLogin) {
                              setTask((t) => ({
                                ...t,
                                status: "error",
                                error: e?.message ?? String(e),
                              }));
                              return;
                            }
                            await invoke("sync_latest_install_from_manifest", {
                              version: v,
                            });
                            return;
                          } catch {}
                        }
                        setTask((t) => ({
                          ...t,
                          status: "error",
                          error: e?.message ?? String(e),
                        }));
                      }
                    }}
                  >
                    Update
                  </Button>
                </>
              ) : (
                <Button
                  variant="secondary"
                  className="h-10 min-w-[120px]"
                  onClick={() => {
                    setUpdatePrompt({ open: false });
                    if (!updateIsWorking) {
                      setManifestUpdateInfo(null);
                    }
                  }}
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

          <div className="relative w-[min(520px,calc(100vw-2rem))] rounded-2xl border border-panel-outline bg-[#0f1116] p-5 shadow-2xl shadow-black/50">
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

            <div className="mt-4 rounded-2xl border border-panel-outline bg-white/5 p-3">
              <div className="flex items-center justify-between gap-3">
                <div className="min-w-0">
                  <div className="truncate text-sm font-semibold">
                    {practiceTask?.step_name ?? "Practice Mods"}
                  </div>
                  <div className="truncate text-xs text-white/50">
                    {practiceTask?.detail ?? ""}
                  </div>
                  {(formatTransferProgress(practiceTask) ||
                    formatExtractProgress(practiceTask)) && (
                    <div className="mt-1 text-xs text-white/40">
                      {[
                        formatTransferProgress(practiceTask),
                        formatExtractProgress(practiceTask),
                      ]
                        .filter(Boolean)
                        .join(" • ")}
                    </div>
                  )}
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
                disabled={practiceCancelBusy}
                onClick={async () => {
                  const st = practiceTask?.status ?? "working";
                  if (st === "working") {
                    explicitCancelKeyRef.current = latestPrepareKeyRef.current;
                    const v = Number(practiceTask?.version ?? selectedVersion);
                    if (!Number.isFinite(v)) return;
                    setPracticeCancelBusy(true);
                    setPracticeTask((t) => ({
                      ...(t ?? {}),
                      status: "working",
                      detail: "Cancelling...",
                      error: null,
                    }));
                    try {
                      await invoke("cancel_prepare", { version: v });
                    } catch (e) {
                      console.error(e);
                      setPracticeCancelBusy(false);
                    }
                  } else {
                    setPracticePrompt({ open: false });
                  }
                }}
              >
                {(practiceTask?.status ?? "working") === "working"
                  ? practiceCancelBusy
                    ? "Cancelling..."
                    : "Cancel"
                  : "Close"}
              </Button>
            </div>
          </div>
        </div>
      )}

      {/* Preset install modal (only shows when installing preset/tagged plugins) */}
      {presetPrompt.open && (
        <div className="fixed inset-0 z-50 flex items-center justify-center">
          <button
            className="absolute inset-0 bg-black/60"
            onClick={() => {
              const st = presetTask?.status ?? "working";
              if (st !== "working") setPresetPrompt({ open: false });
            }}
            aria-label="Close"
          />

          <div className="relative w-[min(520px,calc(100vw-2rem))] rounded-2xl border border-panel-outline bg-[#0f1116] p-5 shadow-2xl shadow-black/50">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <div className="text-lg font-semibold">
                  {presetTask?.status === "error"
                    ? "Preset setup failed"
                    : "Installing preset mods..."}
                </div>
                <div className="mt-1 text-sm text-white/55">
                  Preset-tagged plugins are being installed for this run.
                </div>
              </div>
            </div>

            <div className="mt-4 rounded-2xl border border-panel-outline bg-white/5 p-3">
              <div className="flex items-center justify-between gap-3">
                <div className="min-w-0">
                  <div className="truncate text-sm font-semibold">
                    {presetTask?.step_name ?? "Preset Mods"}
                  </div>
                  <div className="truncate text-xs text-white/50">
                    {presetTask?.detail ?? ""}
                  </div>
                  {(formatTransferProgress(presetTask) ||
                    formatExtractProgress(presetTask)) && (
                    <div className="mt-1 text-xs text-white/40">
                      {[
                        formatTransferProgress(presetTask),
                        formatExtractProgress(presetTask),
                      ]
                        .filter(Boolean)
                        .join(" • ")}
                    </div>
                  )}
                  {presetTask?.error && (
                    <div className="mt-1 text-xs text-red-300">
                      {presetTask.error}
                    </div>
                  )}
                </div>
                <div className="shrink-0 text-sm text-white/70">
                  {Number.isFinite(Number(presetTask?.overall_percent))
                    ? `${Math.round(Number(presetTask?.overall_percent))}%`
                    : ""}
                </div>
              </div>
              <div className="mt-2 h-2 w-full overflow-hidden rounded-full bg-white/10">
                <div
                  className={cn(
                    "h-full rounded-full transition-[width]",
                    presetTask?.status === "error" ? "bg-red-400" : "bg-emerald-400"
                  )}
                  style={{
                    width: `${Math.max(
                      0,
                      Math.min(100, Number(presetTask?.overall_percent ?? 0))
                    )}%`,
                  }}
                />
              </div>
            </div>

            <div className="mt-5 flex items-center justify-end gap-2">
              <Button
                variant="secondary"
                className="h-10 min-w-[120px]"
                disabled={presetCancelBusy}
                onClick={async () => {
                  const st = presetTask?.status ?? "working";
                  if (st === "working") {
                    explicitCancelKeyRef.current = latestPrepareKeyRef.current;
                    const v = Number(presetTask?.version ?? selectedVersion);
                    if (!Number.isFinite(v)) return;
                    setPresetCancelBusy(true);
                    setPresetTask((t) => ({
                      ...(t ?? {}),
                      status: "working",
                      detail: "Cancelling...",
                      error: null,
                    }));
                    try {
                      await invoke("cancel_prepare", { version: v });
                    } catch (e) {
                      console.error(e);
                      setPresetCancelBusy(false);
                    }
                  } else {
                    setPresetPrompt({ open: false });
                  }
                }}
              >
                {(presetTask?.status ?? "working") === "working"
                  ? presetCancelBusy
                    ? "Cancelling..."
                    : "Cancel"
                  : "Close"}
              </Button>
            </div>
          </div>
        </div>
      )}

      <Dialog
        open={steamOverlayDialogOpen}
        onOpenChange={(open) => {
          if (steamOverlaySaveBusy) return;
          setSteamOverlayDialogOpen(open);
          if (!open) {
            setSteamOverlayError("");
            setSteamOverlaySaved("");
          }
        }}
      >
        <DialogContent className="w-[min(640px,92vw)] p-0">
          <div className="rounded-3xl border border-panel-outline bg-[#0f1116] p-6 text-white">
            <div className="flex items-start justify-between gap-4">
              <div>
                <div className="text-lg font-semibold tracking-[-0.02em]">
                  Inject Steam Overlay
                </div>
                <div className="mt-1 text-sm text-white/55">
                  Toggle Steam Overlay DLL injection and optionally override the Steam install path.
                </div>
              </div>
              <button
                type="button"
                className="rounded-xl border border-panel-outline bg-white/5 p-2 text-white/70 transition hover:bg-white/10 hover:text-white"
                onClick={() => {
                  if (steamOverlaySaveBusy) return;
                  setSteamOverlayDialogOpen(false);
                  setSteamOverlayError("");
                  setSteamOverlaySaved("");
                }}
                aria-label="Close steam overlay settings"
              >
                <X className="h-4 w-4" />
              </button>
            </div>

            <div className="mt-6 space-y-5">
              <div className="flex items-center justify-between gap-4 rounded-2xl border border-panel-outline bg-white/[0.04] px-4 py-3">
                <div>
                  <div className="text-sm font-medium text-white">
                    Enable Inject Steam Overlay
                  </div>
                  <div className="mt-1 text-xs text-white/50">
                    When enabled, the launcher starts the game suspended, injects the Steam overlay DLLs, then resumes the process.
                  </div>
                </div>
                <Switch
                  checked={steamOverlayConfig.enabled}
                  disabled={steamOverlaySaveBusy}
                  onCheckedChange={(checked) => {
                    setSteamOverlayConfig((prev) => ({ ...prev, enabled: checked }));
                    setSteamOverlaySaved("");
                  }}
                />
              </div>

              <div>
                <label
                  htmlFor="steam-overlay-path"
                  className="mb-2 block text-sm font-medium text-white/80"
                >
                  Steam path override
                </label>
                <div className="flex items-center gap-2">
                  <Input
                    id="steam-overlay-path"
                    value={steamOverlayConfig.steam_path}
                    disabled={steamOverlaySaveBusy}
                    onChange={(event) => {
                      setSteamOverlayConfig((prev) => ({
                        ...prev,
                        steam_path: event.target.value,
                      }));
                      setSteamOverlaySaved("");
                    }}
                    placeholder="Leave blank to auto-detect Steam"
                  />
                  <Button
                    variant="secondary"
                    className="h-10 shrink-0"
                    disabled={steamOverlaySaveBusy}
                    onClick={() => {
                      browseSteamOverlayPath().catch(console.error);
                    }}
                  >
                    Browse
                  </Button>
                </div>
                <div className="mt-2 text-xs text-white/45">
                  {steamOverlayResolvedPath ? (
                    <>
                      Auto-detected path:{" "}
                      <span className="font-mono">{steamOverlayResolvedPath}</span>
                    </>
                  ) : (
                    <>
                      Example: <span className="font-mono">C:\Program Files (x86)\Steam</span>
                    </>
                  )}
                </div>
              </div>

              {steamOverlayError ? (
                <div className="rounded-2xl border border-red-400/30 bg-red-400/10 px-4 py-3 text-sm text-red-200">
                  {steamOverlayError}
                </div>
              ) : null}

              {steamOverlaySaved ? (
                <div className="rounded-2xl border border-emerald-400/30 bg-emerald-400/10 px-4 py-3 text-sm text-emerald-200">
                  {steamOverlaySaved}
                </div>
              ) : null}
            </div>

            <div className="mt-6 flex items-center justify-end gap-2">
              <Button
                variant="secondary"
                className="h-10 min-w-[96px]"
                disabled={steamOverlaySaveBusy}
                onClick={() => {
                  setSteamOverlayDialogOpen(false);
                  setSteamOverlayError("");
                  setSteamOverlaySaved("");
                }}
              >
                Close
              </Button>
              <Button
                variant="default"
                className="h-10 min-w-[120px]"
                disabled={steamOverlaySaveBusy}
                onClick={() => {
                  saveSteamOverlaySettings().catch(console.error);
                }}
              >
                {steamOverlaySaveBusy ? "Saving..." : "Save"}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}
