import { useEffect, useMemo, useRef, useState, useCallback } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import {
  ArrowLeftRight,
  Check,
  CheckCircle2,
  ChevronDown,
  Download,
  FileCog,
  FolderOpen,
  Globe2,
  Info,
  LogOut,
  LoaderCircle,
  MessageCircle,
  Play,
  RefreshCw,
  Search,
  Timer,
  Trash2,
  X,
} from "lucide-react";
import { Button } from "../components/ui/button";
import { Dialog, DialogContent } from "../components/ui/dialog";
import { Input } from "../components/ui/input";
import { Checkbox } from "../components/ui/checkbox";
import { Switch } from "../components/ui/switch";
import { Slider } from "../components/ui/slider";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "../components/ui/select";
import { cn } from "../lib/cn";
import { ModCover } from "../components/launcher/ModCover";
import { ScrollableDropdownContent } from "../components/launcher/ScrollableDropdownContent";
import { LauncherPageSkeleton } from "../components/launcher/Skeletons";
import { isAuthError } from "../lib/errors";
import { clamp, fmtBytes, formatExtractProgress, formatTransferProgress, sleep, valueLabel } from "../lib/format";
import { BASE_VLOG_MOD_KEY, DEFAULT_MOD_PANEL_WIDTH, DISCORD_DOWNLOAD_URL, ECLIPSED_FORCED_MOD_KEYS, ECLIPSED_HQ_OPTIONAL_MOD_KEYS, EVENT_VLOG_MOD, LAUNCH_OPTIONS_STORAGE_KEY, MIN_CONFIG_PANEL_WIDTH, MIN_MOD_PANEL_WIDTH, MOD_PANEL_WIDTH_STORAGE_KEY, PRACTICE_LOCKED_MOD_KEYS, RUN_MODE_VALUES, SMHQ_FORCED_MOD_KEYS, RUN_OPTIONS } from "./launcher/constants";
import { clampVersionToEvent, eventAllowsTester, eventAllowsVersion, formatEventTimeRemaining, getInitialEventsEnabled, getInitialSelectedEventId, isEventActive, normalizeEventPreset, parseUtcEventTime, saveEventsEnabled, saveSelectedEventId, saveSelectedVersion } from "./launcher/events";
import { extractSpreadsheetId, findSheetInfoByTitle, findSheetTitleByGid, findSimilarSheetInfoByTitle, normalizeSheetColumn, normalizeSheetColumnList, normalizeSheetInfos, parseSpreadsheetInput } from "./launcher/googleSheets";
import { getInitialLaunchOptionsConfig, makeDeleteVersionPromptState, normalizeLaunchCommandTemplate, normalizeLaunchOptionsEntries } from "./launcher/launchOptions";
import { CUSTOM_TIME_FORMAT_OPTIONS, DEFAULT_CUSTOM_LCSTATS_LAYOUT, DEFAULT_LCSTATS_SETTINGS, LCSTATS_LAYOUTS, hasCustomGoogleOauthSettings, isLcStatsTrackerMod, lcstatsLayoutUsesColumnFields, normalizeCustomLcstatsLayout, parseCustomLcstatsLayoutPreset, requiresGoogleOauthForMod } from "./launcher/lcstats";
import { configPathMatchesMod, isLockedCfgEntry, isModCompatibleWithTags, isModCompatibleWithVersion, isPresetSummaryMod, isUiHiddenMod, listEntryKey, modHasRunModeAffinity, modKey, modKeyLower, filterForcedMods
} from "./launcher/mods";
import { clampVersionToRange, getInitialRunMode, getLaunchRequestForRunMode, getPresetModulePriority, getPresetSummarySpec, getPresetVersionRange, isEclipsedHqRunMode, isEclipsedRunMode, isPracticeRunMode, isSmhqRunMode, isVersionWithinRange, saveSelectedRunMode, shouldResetFreeMoonsOnRunModeChange } from "./launcher/runModes";
import { useDismissableContextMenu } from "../hooks/useContextMenu";
import { ProgressBar, TaskProgressPanel } from "../components/launcher/TaskProgress";
import { PrepareCancelButton } from "../components/launcher/PrepareCancelButton";
import { reconcileEventsEnabled } from "../lib/eventsSetting";
import { resolveUpdateEvent } from "./launcher/updateTask";





































































































































export default function LauncherPage({
  loginState,
  onLogout,
  onRequireLogin,
  bootstrapError,
  onInstalledVersionsChange,
}) {
  const initialLaunchOptionsConfig = getInitialLaunchOptionsConfig();
  const [installedVersions, setInstalledVersions] = useState([]);
  const [selectedVersion, setSelectedVersion] = useState(null);
  const [manifest, setManifest] = useState({
    version: null,
    mods: [],
    manifests: {},
  });
  const [eventsManifest, setEventsManifest] = useState({
    version: 0,
    events: [],
  });
  const [eventsEnabled, setEventsEnabled] = useState(getInitialEventsEnabled);
  const [eventClock, setEventClock] = useState(() => Date.now());
  const [selectedEventId, setSelectedEventId] = useState(getInitialSelectedEventId);
  const [practiceMods, setPracticeMods] = useState([]);

  const [query, setQuery] = useState("");
  const [selectedMod, setSelectedMod] = useState(null);
  const [modEnabled, setModEnabled] = useState(true);
  const [modToggleBusy, setModToggleBusy] = useState(false);
  const [lcstatsTrackingBusy, setLcstatsTrackingBusy] = useState(false);
  const [lcstatsTrackingEnabled, setLcstatsTrackingEnabled] = useState(false);
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
  const [launchOptionsDialogOpen, setLaunchOptionsDialogOpen] = useState(false);
  const [googleOauthDialogOpen, setGoogleOauthDialogOpen] = useState(false);
  const [googleOauthBusy, setGoogleOauthBusy] = useState(false);
  const [googleOauthError, setGoogleOauthError] = useState("");
  const [customGoogleOauthOpen, setCustomGoogleOauthOpen] = useState(false);
  const [customLayoutSectionOpen, setCustomLayoutSectionOpen] = useState({
    Rows: true,
    Run: true,
    Scrap: true,
    Events: false,
    Players: true,
  });
  const [customLayoutClipboardValid, setCustomLayoutClipboardValid] = useState(false);
  const [customLayoutMessage, setCustomLayoutMessage] = useState("");
  const [customLayoutMessageKind, setCustomLayoutMessageKind] = useState("success");
  const [customLayoutSearch, setCustomLayoutSearch] = useState("");
  const [googleOauthStatus, setGoogleOauthStatus] = useState({
    authenticated: false,
    scope: null,
    expires_at: null,
  });
  const [pendingGoogleOauthToggle, setPendingGoogleOauthToggle] = useState(null);
  const [lcstatsSettings, setLcstatsSettings] = useState(DEFAULT_LCSTATS_SETTINGS);
  const [lcstatsSpreadsheets, setLcstatsSpreadsheets] = useState([]);
  const [lcstatsSpreadsheetName, setLcstatsSpreadsheetName] = useState("");
  const [lcstatsSpreadsheetFocused, setLcstatsSpreadsheetFocused] = useState(false);
  const [lcstatsSheets, setLcstatsSheets] = useState([]);
  const [lcstatsSheetInfos, setLcstatsSheetInfos] = useState([]);
  const [lcstatsBusy, setLcstatsBusy] = useState(false);
  const [lcstatsResetBusy, setLcstatsResetBusy] = useState(false);
  const lcstatsResetInFlight = useRef(false);
  const [lcstatsResetEndRow, setLcstatsResetEndRow] = useState("93");
  const [lcstatsRefreshBusy, setLcstatsRefreshBusy] = useState(false);
  const [lcstatsPickerBusy, setLcstatsPickerBusy] = useState(false);
  const lcstatsSaveTimerRef = useRef(null);
  const lcstatsAutoBrowseKeyRef = useRef("");
  const lcstatsAutoSheetKeyRef = useRef("");
  const googleOauthRequestIdRef = useRef(0);
  const [lcstatsSaved, setLcstatsSaved] = useState("");
  const [lcstatsError, setLcstatsError] = useState("");
  const [launchOptionsEnabled, setLaunchOptionsEnabled] = useState(
    initialLaunchOptionsConfig.enabled
  );
  const [launchOptionsEntries, setLaunchOptionsEntries] = useState(
    initialLaunchOptionsConfig.entries
  );
  const [launchCommandTemplate, setLaunchCommandTemplate] = useState(
    initialLaunchOptionsConfig.commandTemplate
  );
  const [newLaunchOptionEntry, setNewLaunchOptionEntry] = useState("");
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
  const [nativeOverlayUpdate, setNativeOverlayUpdate] = useState(null);
  const [nativeOverlayInstalling, setNativeOverlayInstalling] = useState(false);
  const [nativeOverlayUpdateError, setNativeOverlayUpdateError] = useState("");
  const nativeOverlayCheckRequestRef = useRef(0);

  const [gameStatus, setGameStatus] = useState({ running: false, pid: null });
  const [runningGamesDialogOpen, setRunningGamesDialogOpen] = useState(false);
  const [runningGames, setRunningGames] = useState([]);
  const [runningGameStopBusyId, setRunningGameStopBusyId] = useState(null);
  const [runMode, setRunMode] = useState(getInitialRunMode); // hq | practice | brutal | brutal_smhq | brutal_practice | wesley | wesley_practice | smhq
  const [didLoadPersistedRunMode, setDidLoadPersistedRunMode] = useState(false);
  const [hasPersistedRunMode, setHasPersistedRunMode] = useState(false);
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
  const [launchContextMenu, setLaunchContextMenu] = useState({
    open: false,
    x: 0,
    y: 0,
  });
  const practicePromptOpenRef = useRef(false);
  const presetPromptOpenRef = useRef(false);
  const practiceTaskRef = useRef(null);
  const presetTaskRef = useRef(null);
  const runModeRef = useRef(runMode);
  const selectedEventIdRef = useRef(selectedEventId);
  const selectedVersionRef = useRef(selectedVersion);
  const userSelectedRunModeRef = useRef(false);
  const modContextMenuRef = useRef(null);
  const versionContextMenuRef = useRef(null);
  const launchContextMenuRef = useRef(null);
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
    selectedEventIdRef.current = selectedEventId;
  }, [selectedEventId]);
  useEffect(() => {
    selectedVersionRef.current = selectedVersion;
  }, [selectedVersion]);
  useEffect(() => {
    if (gameStatus.running) {
      setLaunchBusy(false);
    }
  }, [gameStatus.running]);

  useEffect(() => {
    let cancelled = false;
    invoke("get_selected_run_mode")
      .then((savedRunMode) => {
        if (cancelled) return;
        if (!userSelectedRunModeRef.current && RUN_MODE_VALUES.includes(savedRunMode)) {
          setHasPersistedRunMode(true);
          setRunMode(savedRunMode);
          localStorage.setItem("selectedRunMode", savedRunMode);
        }
      })
      .catch(() => {})
      .finally(() => {
        if (!cancelled) setDidLoadPersistedRunMode(true);
      });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (lcstatsSettings.layout !== "Custom Layout") {
      setCustomLayoutClipboardValid(false);
      return;
    }
    refreshCustomLayoutClipboardStatus().catch(() => {});
    const timer = window.setInterval(() => {
      refreshCustomLayoutClipboardStatus().catch(() => {});
    }, 1500);
    return () => window.clearInterval(timer);
  }, [lcstatsSettings.layout]);

  useEffect(() => {
    if (typeof window === "undefined") return;
    localStorage.setItem(
      MOD_PANEL_WIDTH_STORAGE_KEY,
      String(modPanelWidthPercent)
    );
  }, [modPanelWidthPercent]);

  useEffect(() => {
    if (typeof window === "undefined") return;
    localStorage.setItem(
      LAUNCH_OPTIONS_STORAGE_KEY,
      JSON.stringify({
        enabled: launchOptionsEnabled,
        entries: normalizeLaunchOptionsEntries(launchOptionsEntries),
        commandTemplate: normalizeLaunchCommandTemplate(launchCommandTemplate),
      })
    );
  }, [launchOptionsEnabled, launchOptionsEntries, launchCommandTemplate]);

  useEffect(() => {
    let cancelled = false;
    invoke("google_lcstats_auth_status")
      .then((status) => {
        if (cancelled) return;
        setGoogleOauthStatus({
          authenticated: !!status?.authenticated,
          scope: status?.scope ?? null,
          expires_at: status?.expires_at ?? null,
        });
      })
      .catch((error) => {
        if (cancelled) return;
        console.error(error);
      });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    invoke("get_lcstats_settings")
      .then((settings) => {
        if (cancelled) return;
        setLcstatsSettings(normalizeLcstatsSettings(settings));
      })
      .catch((error) => {
        if (cancelled) return;
        console.error(error);
      });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!isLcStatsTrackerMod(selectedMod)) return;
    let cancelled = false;
    invoke("get_lcstats_autosheet_tracking")
      .then((running) => {
        if (cancelled) return;
        setLcstatsTrackingEnabled(!!running);
      })
      .catch((error) => {
        if (cancelled) return;
        console.error(error);
      });

    return () => {
      cancelled = true;
    };
  }, [selectedMod]);

  useEffect(() => {
    if (!isLcStatsTrackerMod(selectedMod)) return;
    if (lcstatsSaveTimerRef.current) {
      window.clearTimeout(lcstatsSaveTimerRef.current);
    }
    lcstatsSaveTimerRef.current = window.setTimeout(() => {
      persistLcstatsSettings(lcstatsSettings, { quiet: true }).catch(console.error);
      lcstatsSaveTimerRef.current = null;
    }, 350);

    return () => {
      if (lcstatsSaveTimerRef.current) {
        window.clearTimeout(lcstatsSaveTimerRef.current);
        lcstatsSaveTimerRef.current = null;
      }
    };
  }, [selectedMod, lcstatsSettings]);

  useEffect(() => {
    if (!isLcStatsTrackerMod(selectedMod)) return;
    if (!googleOauthStatus.authenticated) return;
    const key = `${selectedVersion ?? ""}::${googleOauthStatus.expires_at ?? ""}`;
    if (lcstatsAutoBrowseKeyRef.current === key) return;
    lcstatsAutoBrowseKeyRef.current = key;
    refreshLcstatsGoogleLists({ quiet: true }).catch(console.error);
  }, [selectedMod, selectedVersion, googleOauthStatus.authenticated, googleOauthStatus.expires_at]);

  useEffect(() => {
    let unlisten = null;
    (async () => {
      unlisten = await listen("ui://open-launch-options", () => {
        setLaunchOptionsDialogOpen(true);
      });
    })();

    return () => {
      if (typeof unlisten === "function") unlisten();
    };
  }, []);

  useEffect(() => {
    let unlisten = null;
    (async () => {
      unlisten = await listen("ui://events-enabled-changed", (event) => {
        const enabled = !!event.payload?.enabled;
        setEventsEnabled(enabled);
        saveEventsEnabled(enabled);
        setEventClock(Date.now());
      });
    })();

    return () => {
      if (typeof unlisten === "function") unlisten();
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    invoke("get_events_enabled")
      .then((enabled) => {
        if (cancelled) return;
        setEventsEnabled(reconcileEventsEnabled(enabled));
        setEventClock(Date.now());
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!isResizingPanels) return;

    const prevCursor = document.body.style.cursor;
    const prevUserSelect = document.body.style.userSelect;
    document.body.style.cursor = "ew-resize";
    document.body.style.userSelect = "none";
    document.body.classList.add("is-resizing-panels");

    return () => {
      document.body.style.cursor = prevCursor;
      document.body.style.userSelect = prevUserSelect;
      document.body.classList.remove("is-resizing-panels");
    };
  }, [isResizingPanels]);

  useDismissableContextMenu(modContextMenu, setModContextMenu, modContextMenuRef, { mod: null });

  useDismissableContextMenu(versionContextMenu, setVersionContextMenu, versionContextMenuRef, { version: null });

  useDismissableContextMenu(launchContextMenu, setLaunchContextMenu, launchContextMenuRef);

  const isInstalled = useMemo(() => {
    const s = new Set(installedVersions);
    return (v) => s.has(v);
  }, [installedVersions]);

  const activeEvents = useMemo(() => {
    if (!eventsEnabled) return [];
    const events = Array.isArray(eventsManifest?.events) ? eventsManifest.events : [];
    return events.filter((event) => {
      if (!event?.id || !event?.name) return false;
      const testerAllowed = eventAllowsTester(event, loginState);
      return isEventActive(event, eventClock, testerAllowed);
    });
  }, [eventClock, eventsEnabled, eventsManifest, loginState]);

  const selectedEvent = useMemo(() => {
    if (!selectedEventId) return null;
    return (
      activeEvents.find(
        (event) =>
          String(event.id).toLowerCase() === String(selectedEventId).toLowerCase()
      ) ?? null
    );
  }, [activeEvents, selectedEventId]);

  const selectedEventRunMode = useMemo(
    () => normalizeEventPreset(selectedEvent?.preset),
    [selectedEvent]
  );
  const selectedEventTimeRemaining = useMemo(
    () => formatEventTimeRemaining(selectedEvent, eventClock),
    [eventClock, selectedEvent]
  );

  useEffect(() => {
    const endsAt = parseUtcEventTime(selectedEvent?.ends_at);
    const remainingMs = endsAt === null ? null : endsAt - Date.now();
    const intervalMs =
      remainingMs !== null && remainingMs > 0 && remainingMs < 86_400_000
        ? 1000
        : 60_000;
    const timer = window.setInterval(() => setEventClock(Date.now()), intervalMs);
    return () => window.clearInterval(timer);
  }, [selectedEvent?.ends_at]);

  const selectedEventMods = useMemo(() => {
    if (!selectedEvent) return [];
    const mods = Array.isArray(selectedEvent.mods)
      ? [...selectedEvent.mods]
      : [];
    const eventVlogIndex = mods.findIndex(
      (mod) => modKeyLower(mod) === modKeyLower(EVENT_VLOG_MOD)
    );
    if (eventVlogIndex >= 0) {
      const [eventVlog] = mods.splice(eventVlogIndex, 1);
      mods.unshift(eventVlog);
    } else {
      mods.unshift(EVENT_VLOG_MOD);
    }
    return mods.filter(
      (mod) =>
        modKeyLower(mod) !== BASE_VLOG_MOD_KEY &&
        mod?.enabled !== false &&
        !isUiHiddenMod(mod) &&
        isModCompatibleWithVersion(mod, selectedVersion)
    );
  }, [selectedEvent, selectedVersion]);

  const eventForcedModKeys = useMemo(
    () => new Set(selectedEventMods.map((mod) => modKeyLower(mod))),
    [selectedEventMods]
  );

  useEffect(() => {
    if (!didFinishBootstrap || !selectedEventId) return;
    if (selectedEvent) return;

    const version = Number(selectedVersion);
    setSelectedEventId("");
    saveSelectedEventId("");
    if (Number.isFinite(version)) {
      invoke("clear_selected_event", { version })
        .then(() => invoke("get_disabled_mods"))
        .then((dm) => setDisabledMods(Array.isArray(dm) ? dm : []))
        .catch((e) => console.error(e));
    }
  }, [didFinishBootstrap, selectedEvent, selectedEventId, selectedVersion]);

  useEffect(() => {
    if (!didFinishBootstrap) return;
    const version = Number(selectedVersion);
    if (!Number.isFinite(version)) return;

    invoke("reconcile_selected_event", {
      version,
      eventId: selectedEvent?.id ?? null,
    })
      .then((cleared) => {
        if (!cleared) return;
        return invoke("get_disabled_mods").then((dm) =>
          setDisabledMods(Array.isArray(dm) ? dm : [])
        );
      })
      .catch((e) => console.error(e));
  }, [didFinishBootstrap, selectedEvent, selectedVersion]);

  useEffect(() => {
    if (!didFinishBootstrap || !selectedEvent) return;
    if (runMode === selectedEventRunMode) return;
    setRunMode(selectedEventRunMode);
    saveSelectedRunMode(selectedEventRunMode);
  }, [didFinishBootstrap, runMode, selectedEvent, selectedEventRunMode]);

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
        out[key] = path.startsWith("data:") ? path : convertFileSrc(path);
      }
    }
    return out;
  }, [installedModIcons]);

  const forgetInstalledModIcon = useCallback(
    (keyLower) => {
      const version = Number(selectedVersion);
      if (!Number.isFinite(version) || !keyLower) return;
      setInstalledModIconsByVersion((prev) => {
        const current = prev?.[version];
        if (!current || typeof current !== "object" || !(keyLower in current)) {
          return prev;
        }
        const nextForVersion = { ...current };
        delete nextForVersion[keyLower];
        return { ...prev, [version]: nextForVersion };
      });
    },
    [selectedVersion]
  );

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
          !ECLIPSED_FORCED_MOD_KEYS.has(modKeyLower(mod)) &&
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

  const smhqReferenceMods = useMemo(
    () =>
      isSmhqRunMode(runMode)
        ? filterForcedMods(manifest.mods, SMHQ_FORCED_MOD_KEYS, selectedVersion)
        : [],
    [manifest.mods, runMode, selectedVersion]
  );

  const eclipsedReferenceMods = useMemo(
    () =>
      isEclipsedRunMode(runMode)
        ? filterForcedMods(manifest.mods, ECLIPSED_FORCED_MOD_KEYS, selectedVersion)
        : [],
    [manifest.mods, runMode, selectedVersion]
  );

  const eclipsedHqOptionalMods = useMemo(
    () =>
      isEclipsedHqRunMode(runMode)
        ? filterForcedMods(
            manifest.mods,
            ECLIPSED_HQ_OPTIONAL_MOD_KEYS,
            selectedVersion
          )
        : [],
    [manifest.mods, runMode, selectedVersion]
  );

  const modsForList = useMemo(() => {
    const regularMods = (Array.isArray(manifest.mods) ? manifest.mods : []).filter(
      (m) =>
        !(selectedEvent && modKeyLower(m) === BASE_VLOG_MOD_KEY) &&
        modKeyLower(m) !== modKeyLower(EVENT_VLOG_MOD) &&
        !modHasRunModeAffinity(m) &&
        m?.enabled !== false &&
        isModCompatibleWithVersion(m, selectedVersion)
    );
    const merged = [
      ...(isPracticeRunMode(runMode) ? practiceReferenceMods : []),
      ...selectedEventMods,
      ...smhqReferenceMods,
      ...eclipsedReferenceMods,
      ...eclipsedHqOptionalMods,
      ...regularMods,
    ];
    if (
      !selectedEvent &&
      !isPracticeRunMode(runMode) &&
      !isSmhqRunMode(runMode) &&
      !isEclipsedRunMode(runMode)
    ) {
      return regularMods;
    }

    const seen = new Set();
    return merged.filter((m) => {
      const key = modKeyLower(m);
      if (!key || seen.has(key)) return false;
      seen.add(key);
      return true;
    });
  }, [
    eclipsedHqOptionalMods,
    eclipsedReferenceMods,
    manifest.mods,
    practiceReferenceMods,
    runMode,
    selectedEvent,
    selectedEventMods,
    selectedVersion,
    smhqReferenceMods,
  ]);

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

  const eclipsedForcedModKeys = useMemo(() => {
    if (!isEclipsedRunMode(runMode)) return new Set();
    return new Set(eclipsedReferenceMods.map((m) => modKeyLower(m)));
  }, [eclipsedReferenceMods, runMode]);

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
    const switchDisabledSet = new Set(
      disabledMods.map(
        (mod) =>
          `${String(mod?.dev ?? "").toLowerCase()}::${String(
            mod?.name ?? ""
          ).toLowerCase()}`
      )
    );
    const switchRepresentativeByKey = new Map();
    const switchGroups = new Map();
    for (const mod of modsForList) {
      const group = String(mod?.switch_group ?? "").trim().toLowerCase();
      if (!group) continue;
      switchGroups.set(group, [...(switchGroups.get(group) ?? []), mod]);
    }
    for (const [group, pairMods] of switchGroups) {
      if (pairMods.length < 2) continue;
      const enabledMod = pairMods.find(
        (mod) => !switchDisabledSet.has(modKeyLower(mod))
      );
      const selectedPairMod = pairMods.find(
        (mod) => selectedMod && modKeyLower(mod) === modKeyLower(selectedMod)
      );
      const installedMod = pairMods.find((mod) => !!installedModVersions[modKeyLower(mod)]);
      const representative = enabledMod ?? selectedPairMod ?? installedMod ?? pairMods[0];
      const entry = {
        ...representative,
        _modSwitchPair: pairMods,
        _modSwitchGroup: group,
      };
      for (const mod of pairMods) {
        switchRepresentativeByKey.set(modKeyLower(mod), entry);
      }
    }

    const mods = [];
    const addedSwitchPairs = new Set();
    for (const mod of modsForList) {
      const key = modKeyLower(mod);
      const representative = switchRepresentativeByKey.get(key);
      if (!representative) {
        mods.push(mod);
        continue;
      }
      const pairId = representative._modSwitchGroup;
      if (addedSwitchPairs.has(pairId)) continue;
      addedSwitchPairs.add(pairId);
      mods.push(representative);
    }

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
          return configPathMatchesMod(p, m);
        });
        if (hit) return canonicalChainId(chain);
      }
      return null;
    };

    const matchesQuery = (m) => {
      // Preserve existing special-case exclusion
      if (String(m?.name ?? "") === "ShipLootCruiser") return false;
      if (!q) return true;
      const hay = (Array.isArray(m?._modSwitchPair) ? m._modSwitchPair : [m])
        .map((entry) => `${entry.dev} ${entry.name}`)
        .join(" ")
        .toLowerCase();
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
    disabledMods,
  ]);

  const displayedMods = useMemo(() => {
    if (selectedEvent) {
      const eventVlogIndex = filteredMods.findIndex(
        (mod) => modKeyLower(mod) === modKeyLower(EVENT_VLOG_MOD)
      );
      if (eventVlogIndex >= 0) {
        const eventVlog = filteredMods[eventVlogIndex];
        const rest = filteredMods.filter((_, index) => index !== eventVlogIndex);
        return presetSummaryEntry
          ? [eventVlog, presetSummaryEntry, ...rest]
          : [eventVlog, ...rest];
      }
    }
    if (!presetSummaryEntry) return filteredMods;
    return [presetSummaryEntry, ...filteredMods];
  }, [filteredMods, presetSummaryEntry, selectedEvent]);

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
    const v = task.version != null && Number(task.version) > 0 ? ` v${task.version}` : "";
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
      const persistedVersion = await invoke("get_selected_version").catch(() => null);
      const saved = localStorage.getItem("selectedVersion");
      const savedNum = Number.isFinite(Number(persistedVersion))
        ? Number(persistedVersion)
        : saved == null
        ? null
        : Number(saved);

      const manifestPromise = invoke("get_manifest");
      const eventsPromise = invoke("get_events").catch(() => ({ version: 0, events: [] }));
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

        try {
          setBootstrapStatus("Fetching remote events...");
          const ev = await eventsPromise;
          if (!cancelled) {
            setEventsManifest(
              ev && typeof ev === "object" ? ev : { version: 0, events: [] }
            );
          }
        } catch (e) {
          console.error(e);
        }
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

  useEffect(() => {
    let unlisten = null;
    let disposed = false;

    (async () => {
      unlisten = await listen("release-channel://changed", async () => {
        try {
          const [mf, ev] = await Promise.all([
            invoke("get_manifest"),
            invoke("get_events").catch(() => ({ version: 0, events: [] })),
          ]);
          if (disposed) return;

          setManifest(
            mf ?? { version: null, mods: [], manifests: {}, preset_tag_constraints: {} }
          );
          setEventsManifest(
            ev && typeof ev === "object" ? ev : { version: 0, events: [] }
          );
          setSelectedMod(null);
          setManifestUpdateInfo(null);

          const remoteV =
            mf?.manifests && typeof mf.manifests === "object"
              ? Object.keys(mf.manifests)
                  .map((k) => Number(k))
                  .filter((n) => Number.isFinite(n))
              : [];
          remoteV.sort((a, b) => a - b);

          const availableVersions = new Set([...installedVersions, ...remoteV]);
          setSelectedVersion((prev) => {
            const prevNum = Number(prev);
            if (Number.isFinite(prevNum) && availableVersions.has(prevNum)) {
              return prevNum;
            }
            if (installedVersions.length > 0) {
              return installedVersions[installedVersions.length - 1];
            }
            if (remoteV.length > 0) return remoteV[remoteV.length - 1];
            return Number.isFinite(prevNum) ? prevNum : null;
          });
        } catch (e) {
          console.error(e);
        }
      });
    })();

    return () => {
      disposed = true;
      if (typeof unlisten === "function") unlisten();
    };
  }, [installedVersions]);

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
    checkModUpdates(version, { runMode: selectedEvent ? selectedEventRunMode : runMode });
  }, [
    didFinishBootstrap,
    installedVersions,
    isInstalled,
    preparedUpdateContext,
    runMode,
    selectedEvent,
    selectedEventRunMode,
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
        .then((s) => {
          const nextStatus = s ?? { running: false, pid: null };
          setGameStatus(nextStatus);
          if (!nextStatus.running) {
            setLcstatsTrackingEnabled(false);
          }
        })
        .catch(() => {});
    }, 1500);
    return () => clearInterval(t);
  }, [gameStatus.running]);

  // listen to backend progress events
  useEffect(() => {
    let unlistenProgress = null;
    let unlistenFinished = null;
    let unlistenError = null;
    let unlistenStorageChanged = null;

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
        const isEventModsStep = stepName === "Event Mods";
        const isDeleteStep = stepName === "Delete Version";
        const isStorageMoveStep = stepName === "Move Storage";
        const isManifestSyncStep =
          stepName === "Sync Mods" || stepName === "Sync Game";
        const isSetupStep =
          isEnableModStep ||
          stepName === "Practice Mods" ||
          stepName === "Preset Mods" ||
          isEventModsStep ||
          isModFilesStep ||
          isDeleteStep ||
          isStorageMoveStep;
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
        // Event installs use the same setup progress UI as preset installs.
        if (isEventModsStep) {
          setPresetCancelBusy(false);
          setPresetTask({
            status: didFinish ? "done" : "working",
            ...p,
            error: null,
          });
          setPresetPrompt({ open: true });
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
        if (isEnableModStep || isManifestSyncStep || isStorageMoveStep) {
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
        // The unified update flow may have installed an overlay update too;
        // refresh the overlay state so the UI no longer shows it as pending.
        void refreshNativeOverlayUpdate();
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
      unlistenStorageChanged = await listen("game-storage://changed", () => {
        invoke("list_installed_versions")
          .then((v) => updateInstalledVersionsState(v))
          .catch(() => {});
      });
    })();

    return () => {
      if (typeof unlistenProgress === "function") unlistenProgress();
      if (typeof unlistenFinished === "function") unlistenFinished();
      if (typeof unlistenError === "function") unlistenError();
      if (typeof unlistenStorageChanged === "function") unlistenStorageChanged();
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
          setCheckUpdateTask((t) => {
            const { stale, runMode: eventRunMode } = resolveUpdateEvent(
              t,
              event.payload
            );
            if (stale) return t;

            const total = Number(event.payload?.total ?? 0);
            const checked = Number(event.payload?.checked ?? 0);
            const overall_percent =
              total > 0 && Number.isFinite(total) && Number.isFinite(checked)
                ? (checked / total) * 100
                : 0;
            return {
              ...t,
              status: "working",
              ...event.payload,
              version: event.payload?.version ?? t.version,
              run_mode: eventRunMode,
              overall_percent: overall_percent,
              error: null,
            };
          });
        }
      );
      unlistenCheckUpdateFinished = await listen(
        "updatable://finished",
        (event) => {
          setCheckUpdateTask((t) => {
            const { stale, runMode: eventRunMode } = resolveUpdateEvent(
              t,
              event.payload
            );
            if (stale) return t;

            return {
              ...t,
              status: "done",
              ...event.payload,
              run_mode: eventRunMode,
            };
          });
          // refresh installed versions list after install
          invoke("list_installed_versions")
            .then((v) => updateInstalledVersionsState(v))
            .catch(() => {});
          // Refresh overlay update info so the unified check reflects overlay
          // availability alongside mod updates.
          void refreshNativeOverlayUpdate();
        }
      );
      unlistenCheckUpdateError = await listen("updatable://error", (event) => {
        setCheckUpdateTask((t) => {
          const { stale, runMode: eventRunMode } = resolveUpdateEvent(
            t,
            event.payload
          );
          if (stale) return t;

          return {
            ...t,
            status: "error",
            version: event.payload?.version ?? t.version,
            run_mode: eventRunMode,
            error: event.payload?.message ?? "Unknown error",
          };
        });
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

  useEffect(() => {
    void refreshNativeOverlayUpdate();
  }, []);

  useEffect(() => {
    if (!didFinishBootstrap) return;
    const version = Number(selectedVersion);
    if (!Number.isFinite(version) || !isInstalled(version)) {
      setNativeOverlayUpdate(null);
      return;
    }
    // Do not carry the previous game's proxy status across version changes.
    setNativeOverlayUpdate(null);
    void refreshNativeOverlayUpdate(version);
  }, [didFinishBootstrap, installedVersions, isInstalled, selectedVersion]);

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

    if (isLcStatsTrackerMod(selectedMod)) {
      const key = modKeyLower(selectedMod);
      setConfigFiles([]);
      setActiveConfigPath("");
      setCfgFile(null);
      setActiveSection("");
      setCfgError("");
      setModEnabled(!disabledSet.has(key));
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
        await prepareRunMode(selectedEvent ? selectedEventRunMode : runMode, v, {
          assumeInstalled: true,
          eventId: selectedEvent?.id,
        });
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

  async function refreshNativeOverlayUpdate(version = selectedVersion) {
    const requestId = ++nativeOverlayCheckRequestRef.current;
    try {
      const numericVersion = Number(version);
      const info = await invoke("check_native_overlay_update", {
        version: Number.isFinite(numericVersion) ? numericVersion : null,
      });
      if (requestId === nativeOverlayCheckRequestRef.current) {
        setNativeOverlayUpdate(info ?? null);
      }
      return info;
    } catch (error) {
      console.warn("Failed to check the HQ Overlay version", error);
      return null;
    }
  }

  async function installNativeOverlayUpdate() {
    if (nativeOverlayInstalling) return;
    setNativeOverlayInstalling(true);
    setNativeOverlayUpdateError("");
    try {
      const info = await invoke("install_native_overlay_update");
      setNativeOverlayUpdate(info ?? null);
    } catch (error) {
      setNativeOverlayUpdateError(error?.message ?? String(error));
    } finally {
      setNativeOverlayInstalling(false);
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
      await Promise.all([
        invoke("check_mod_updates", { version: v, runMode: nextRunMode }),
        refreshNativeOverlayUpdate(v),
      ]);
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
      // The unified update also refreshes the selected version's
      // `.hq-overlay-version` stamp. Re-check it before rendering the update
      // state so a stale `available: true` result is not left in the dialog.
      await refreshNativeOverlayUpdate(v);
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
    if (isLockedCfgEntry(activeConfigPath, sectionName, entryName)) return;
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
    const mods = modsForList;

    // Best-effort: narrow down candidates by substring match, then confirm by asking backend.
    const candidates = mods
      .filter((m) => {
        return configPathMatchesMod(p, m);
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

  async function refreshGoogleOauthStatus() {
    const status = await invoke("google_lcstats_auth_status");
    const nextStatus = {
      authenticated: !!status?.authenticated,
      scope: status?.scope ?? null,
      expires_at: status?.expires_at ?? null,
    };
    setGoogleOauthStatus(nextStatus);
    return nextStatus;
  }

  function requestGoogleOauthForToggle(mod, nextEnabled, opts) {
    setPendingGoogleOauthToggle({ mod, nextEnabled: !!nextEnabled, opts });
    setGoogleOauthError("");
    setGoogleOauthDialogOpen(true);
  }

  function cancelGoogleOauthLogin() {
    googleOauthRequestIdRef.current += 1;
    setGoogleOauthBusy(false);
    setGoogleOauthDialogOpen(false);
    setGoogleOauthError("");
    setPendingGoogleOauthToggle(null);
  }

  async function enablePendingLcstatsWithoutGoogle() {
    const pending = pendingGoogleOauthToggle;
    if (!pending?.mod) {
      setGoogleOauthDialogOpen(false);
      return;
    }

    googleOauthRequestIdRef.current += 1;
    setGoogleOauthBusy(true);
    setGoogleOauthError("");
    setPendingGoogleOauthToggle(null);
    setGoogleOauthDialogOpen(false);
    try {
      const nextSettings = normalizeLcstatsSettings({
        ...lcstatsSettings,
        allowWithoutGoogle: true,
      });
      setLcstatsSettings(nextSettings);
      await persistLcstatsSettings(nextSettings, { quiet: true });
      await toggleModEnabledForMod(pending.mod, pending.nextEnabled, {
        ...(pending.opts ?? {}),
        skipGoogleOauthGate: true,
        allowWithoutGoogle: true,
      });
    } finally {
      setGoogleOauthBusy(false);
    }
  }

  async function startGoogleOauthLogin() {
    const requestId = googleOauthRequestIdRef.current + 1;
    googleOauthRequestIdRef.current = requestId;
    setGoogleOauthBusy(true);
    setGoogleOauthError("");
    try {
      await persistLcstatsSettings(lcstatsSettings, { quiet: true });
      if (googleOauthRequestIdRef.current !== requestId) return;
      const status = await invoke("google_lcstats_start_oauth");
      if (googleOauthRequestIdRef.current !== requestId) return;
      const nextStatus = {
        authenticated: !!status?.authenticated,
        scope: status?.scope ?? null,
        expires_at: status?.expires_at ?? null,
      };
      setGoogleOauthStatus(nextStatus);
      if (!nextStatus.authenticated) {
        setGoogleOauthError("Google Sheets file permission was not granted.");
        return;
      }

      const pending = pendingGoogleOauthToggle;
      setPendingGoogleOauthToggle(null);
      setGoogleOauthDialogOpen(false);
      if (pending?.mod) {
        await toggleModEnabledForMod(pending.mod, pending.nextEnabled, {
          ...(pending.opts ?? {}),
          skipGoogleOauthGate: true,
        });
      }
    } catch (e) {
      console.error(e);
      if (googleOauthRequestIdRef.current !== requestId) return;
      setGoogleOauthError(e?.message ?? String(e));
    } finally {
      if (googleOauthRequestIdRef.current === requestId) {
        setGoogleOauthBusy(false);
      }
    }
  }

  async function logoutGoogleOauth() {
    setGoogleOauthBusy(true);
    setGoogleOauthError("");
    try {
      await invoke("google_lcstats_logout");
      await refreshGoogleOauthStatus();
      const dm = await invoke("get_disabled_mods");
      setDisabledMods(Array.isArray(dm) ? dm : []);
    } catch (e) {
      console.error(e);
      setGoogleOauthError(e?.message ?? String(e));
    } finally {
      setGoogleOauthBusy(false);
    }
  }

  function updateLcstatsSettings(patch) {
    setLcstatsSettings((prev) => ({ ...prev, ...patch }));
    if (patch?.useLcstatsApi === false) {
      setLcstatsTrackingEnabled(false);
    }
    setLcstatsSaved("");
    setLcstatsError("");
  }

  function normalizeLcstatsSettings(settings) {
    const layout = LCSTATS_LAYOUTS.includes(settings?.layout)
      ? settings.layout
      : LCSTATS_LAYOUTS[0];
    return {
      ...DEFAULT_LCSTATS_SETTINGS,
      ...(settings ?? {}),
      spreadsheetId: extractSpreadsheetId(settings?.spreadsheetId ?? ""),
      activeSheetId: String(settings?.activeSheetId ?? "").trim(),
      startColumn: lcstatsLayoutUsesColumnFields(layout)
        ? normalizeSheetColumn(settings?.startColumn, "D")
        : "",
      quotaColumn: lcstatsLayoutUsesColumnFields(layout)
        ? normalizeSheetColumn(settings?.quotaColumn, "B")
        : "",
      sellColumn: lcstatsLayoutUsesColumnFields(layout)
        ? normalizeSheetColumn(settings?.sellColumn, "AE")
        : "",
      layout,
      customLayout: normalizeCustomLcstatsLayout(settings?.customLayout),
      googleClientId: String(settings?.googleClientId ?? "").trim(),
      googleClientSecret: String(settings?.googleClientSecret ?? "").trim(),
      googlePickerApiKey: String(settings?.googlePickerApiKey ?? "").trim(),
      googlePickerAppId: String(settings?.googlePickerAppId ?? "").trim(),
      useLcstatsApi: settings?.useLcstatsApi !== false,
      allowWithoutGoogle: !!settings?.allowWithoutGoogle,
    };
  }

  async function persistLcstatsSettings(settings, { quiet = false } = {}) {
    if (!quiet) {
      setLcstatsBusy(true);
      setLcstatsError("");
      setLcstatsSaved("");
    }
    try {
      const normalized = normalizeLcstatsSettings(settings);
      await invoke("set_lcstats_settings", { settings: normalized });
      if (!quiet) {
        setLcstatsSettings(normalized);
        setLcstatsSaved("Saved.");
      }
    } catch (e) {
      console.error(e);
      setLcstatsError(e?.message ?? String(e));
    } finally {
      if (!quiet) setLcstatsBusy(false);
    }
  }

  async function openLcstatsSpreadsheetPicker() {
    if (lcstatsPickerBusy) return;
    if (hasCustomGoogleOauthSettings(lcstatsSettings)) return;
    setLcstatsPickerBusy(true);
    setLcstatsError("");
    setLcstatsSaved("");
    try {
      await persistLcstatsSettings(lcstatsSettings, { quiet: true });
      const status = googleOauthStatus.authenticated
        ? googleOauthStatus
        : await refreshGoogleOauthStatus();
      if (!status.authenticated) {
        setGoogleOauthError("");
        setGoogleOauthDialogOpen(true);
        return;
      }

      const selected = await invoke("google_lcstats_pick_spreadsheet", {
        spreadsheetId: extractSpreadsheetId(lcstatsSettings.spreadsheetId),
      });
      if (!selected?.id) return;

      setLcstatsSpreadsheetName(selected.name ?? "");
      setLcstatsSpreadsheets((prev) => {
        const nextFile = { id: selected.id, name: selected.name ?? selected.id };
        const exists = prev.some((file) => file.id === selected.id);
        if (exists) {
          return prev.map((file) => (file.id === selected.id ? nextFile : file));
        }
        return [nextFile, ...prev];
      });
      setLcstatsSettings((prev) => ({
        ...prev,
        spreadsheetId: selected.id,
        activeSheetId:
          selected.id === extractSpreadsheetId(prev.spreadsheetId)
            ? prev.activeSheetId
            : "",
        activeSheetName:
          selected.id === extractSpreadsheetId(prev.spreadsheetId)
            ? prev.activeSheetName
            : "",
      }));
      lcstatsAutoSheetKeyRef.current = `${selected.id}::`;
      const sheets = await loadLcstatsSheetNames(selected.id, {
        quiet: true,
        sheetGid: "",
        persist: true,
      });
      if (sheets !== null) {
        setLcstatsSaved(
          selected.name ? `Selected ${selected.name}.` : "Spreadsheet selected."
        );
      }
    } catch (e) {
      console.error(e);
      setLcstatsError(e?.message ?? String(e));
    } finally {
      setLcstatsPickerBusy(false);
    }
  }

  async function loadLcstatsSheetNames(spreadsheetIdOverride = null, opts = {}) {
    const parsedInput = parseSpreadsheetInput(
      spreadsheetIdOverride ?? lcstatsSettings.spreadsheetId
    );
    const spreadsheetId = parsedInput.spreadsheetId;
    const sheetGid = opts.sheetGid ?? parsedInput.sheetGid;
    if (!opts.partOfRefresh) setLcstatsRefreshBusy(true);
    if (!opts.quiet) {
      setLcstatsError("");
      setLcstatsSaved("");
    }
    if (!spreadsheetId) {
      setLcstatsSheets([]);
      setLcstatsSheetInfos([]);
      if (!opts.partOfRefresh) setLcstatsRefreshBusy(false);
      return [];
    }
    try {
      if (!googleOauthStatus.authenticated) {
        const status = await refreshGoogleOauthStatus();
        if (!status.authenticated) {
          requestGoogleOauthForToggle(selectedMod, true, {
            skipGoogleOauthGate: true,
          });
          return null;
        }
      }
      const sheets = await invoke("list_lcstats_sheet_infos", {
        spreadsheetId,
      });
      const infos = normalizeSheetInfos(sheets);
      const list = infos.map((sheet) => sheet.title);
      const linkedSheetInfo = sheetGid
        ? infos.find((sheet) => sheet.sheetId === String(sheetGid))
        : null;
      const currentSheetInfo =
        linkedSheetInfo ||
        infos.find((sheet) => sheet.sheetId === lcstatsSettings.activeSheetId) ||
        findSheetInfoByTitle(infos, lcstatsSettings.activeSheetName) ||
        (sheetGid
          ? findSimilarSheetInfoByTitle(infos, lcstatsSettings.activeSheetName)
          : null);
      const activeSheetName =
        currentSheetInfo?.title ||
        (list.includes(lcstatsSettings.activeSheetName)
          ? lcstatsSettings.activeSheetName
          : list[0] || "");
      const activeSheetId =
        currentSheetInfo?.sheetId ||
        findSheetInfoByTitle(infos, activeSheetName)?.sheetId ||
        "";
      setLcstatsSheetInfos(infos);
      setLcstatsSheets(list);
      const nextSettings = normalizeLcstatsSettings({
        ...lcstatsSettings,
        spreadsheetId,
        activeSheetName,
        activeSheetId,
      });
      setLcstatsSettings(nextSettings);
      if (opts.persist) {
        await persistLcstatsSettings(nextSettings, { quiet: true });
      }
      if (!opts.quiet) {
        setLcstatsSaved(list.length > 0 ? "Sheet list loaded." : "No sheets found.");
      }
      return list;
    } catch (e) {
      console.error(e);
      setLcstatsSheets([]);
      setLcstatsSheetInfos([]);
      if (!opts.quiet) setLcstatsError(e?.message ?? String(e));
      return null;
    } finally {
      if (!opts.partOfRefresh) setLcstatsRefreshBusy(false);
    }
  }

  function loadLcstatsSheetNamesFromParsedInput(parsedInput) {
    if (!parsedInput.spreadsheetId) return;
    const key = `${parsedInput.spreadsheetId}::${parsedInput.sheetGid}`;
    const currentKey = lcstatsAutoSheetKeyRef.current;
    if (
      currentKey === key ||
      (!parsedInput.sheetGid && currentKey.startsWith(`${parsedInput.spreadsheetId}::`))
    ) {
      return;
    }
    lcstatsAutoSheetKeyRef.current = key;
    loadLcstatsSheetNames(parsedInput.spreadsheetId, {
      sheetGid: parsedInput.sheetGid,
    }).catch(console.error);
  }

  function handleLcstatsSpreadsheetInputChange(value) {
    const parsedInput = parseSpreadsheetInput(value);
    const currentSpreadsheetId = extractSpreadsheetId(lcstatsSettings.spreadsheetId);
    const spreadsheetChanged = parsedInput.spreadsheetId !== currentSpreadsheetId;
    if (spreadsheetChanged) {
      const matchedFile = lcstatsSpreadsheets.find(
        (file) => file.id === parsedInput.spreadsheetId
      );
      setLcstatsSpreadsheetName(matchedFile?.name ?? "");
    }
    const linkedSheetName = spreadsheetChanged
      ? ""
      : findSheetTitleByGid(lcstatsSheetInfos, parsedInput.sheetGid);
    const patch = {
      spreadsheetId: parsedInput.spreadsheetId,
      activeSheetId: parsedInput.sheetGid || (spreadsheetChanged ? "" : lcstatsSettings.activeSheetId),
    };
    if (spreadsheetChanged || parsedInput.sheetGid) {
      patch.activeSheetName = linkedSheetName;
    }
    updateLcstatsSettings(patch);
    if (spreadsheetChanged) {
      setLcstatsSheets([]);
      setLcstatsSheetInfos([]);
    }
    if (parsedInput.hasSpreadsheetUrl && parsedInput.spreadsheetId) {
      loadLcstatsSheetNamesFromParsedInput(parsedInput);
    }
  }

  async function loadLcstatsSpreadsheets(opts = {}) {
    await refreshLcstatsGoogleLists(opts);
  }

  async function refreshLcstatsGoogleLists(opts = {}) {
    setLcstatsRefreshBusy(true);
    if (!opts.quiet) {
      setLcstatsError("");
      setLcstatsSaved("");
    }
    try {
      if (!googleOauthStatus.authenticated) {
        const status = await refreshGoogleOauthStatus();
        if (!status.authenticated) {
          setGoogleOauthError("");
          setGoogleOauthDialogOpen(true);
          return;
        }
      }
      const parsedInput = parseSpreadsheetInput(lcstatsSettings.spreadsheetId);
      let spreadsheetListError = "";
      let spreadsheets = [];
      try {
        const files = await invoke("list_lcstats_spreadsheets");
        spreadsheets = Array.isArray(files) ? files : [];
      } catch (e) {
        console.error(e);
        spreadsheetListError = e?.message ?? String(e);
      }
      setLcstatsSpreadsheets(spreadsheets);
      if (parsedInput.spreadsheetId) {
        const matchedFile = spreadsheets.find(
          (file) => file.id === parsedInput.spreadsheetId
        );
        if (matchedFile?.name) {
          setLcstatsSpreadsheetName(matchedFile.name);
        }
      }

      let loadedSheets = null;
      if (parsedInput.spreadsheetId) {
        loadedSheets = await loadLcstatsSheetNames(parsedInput.spreadsheetId, {
          quiet: opts.quiet,
          partOfRefresh: true,
          sheetGid: parsedInput.sheetGid,
        });
      } else {
        setLcstatsSheets([]);
        setLcstatsSheetInfos([]);
      }

      if (!opts.quiet) {
        if (spreadsheetListError) {
          if (parsedInput.spreadsheetId && loadedSheets !== null) {
            setLcstatsError("");
            setLcstatsSaved("Spreadsheet list unavailable; sheet list loaded from link/ID.");
          } else if (!parsedInput.spreadsheetId) {
            setLcstatsError(
              `${spreadsheetListError} Paste a Google Sheets link or ID instead.`
            );
          }
        } else if (!parsedInput.spreadsheetId || loadedSheets !== null) {
          setLcstatsSaved(
            spreadsheets.length > 0 ? "Spreadsheet list loaded." : "No spreadsheets found."
          );
        }
      }
    } catch (e) {
      console.error(e);
      setLcstatsError(e?.message ?? String(e));
    } finally {
      setLcstatsRefreshBusy(false);
    }
  }

  async function saveLcstatsSettings() {
    await persistLcstatsSettings(lcstatsSettings);
  }

  function updateCustomLcstatsLayout(patch) {
    updateLcstatsSettings({
      customLayout: normalizeCustomLcstatsLayout({
        ...(lcstatsSettings.customLayout ?? DEFAULT_CUSTOM_LCSTATS_LAYOUT),
        ...patch,
      }),
    });
  }

  async function refreshCustomLayoutClipboardStatus() {
    try {
      const text = await navigator.clipboard.readText();
      setCustomLayoutClipboardValid(!!parseCustomLcstatsLayoutPreset(text));
    } catch {
      setCustomLayoutClipboardValid(false);
    }
  }

  async function copyCustomLcstatsLayout() {
    try {
      const text = JSON.stringify(
        normalizeCustomLcstatsLayout(lcstatsSettings.customLayout),
        null,
        2
      );
      await navigator.clipboard.writeText(text);
      setCustomLayoutClipboardValid(true);
      setCustomLayoutMessage("Custom layout copied.");
      setCustomLayoutMessageKind("success");
      setLcstatsError("");
      setLcstatsSaved("Custom layout copied.");
    } catch (e) {
      console.error(e);
      setCustomLayoutMessage(e?.message ?? String(e));
      setCustomLayoutMessageKind("error");
      setLcstatsSaved("");
      setLcstatsError(e?.message ?? String(e));
    }
  }

  async function loadCustomLcstatsLayoutFromClipboard() {
    try {
      const text = await navigator.clipboard.readText();
      const parsed = parseCustomLcstatsLayoutPreset(text);
      if (!parsed) {
        throw new Error("invalid custom layout preset");
      }
      updateLcstatsSettings({
        customLayout: parsed,
      });
      setCustomLayoutMessage("Custom layout loaded.");
      setCustomLayoutMessageKind("success");
      setLcstatsSaved("Custom layout loaded.");
    } catch (e) {
      console.error(e);
      setCustomLayoutMessage("Clipboard does not contain a valid custom layout JSON.");
      setCustomLayoutMessageKind("error");
      setLcstatsSaved("");
      setLcstatsError("Clipboard does not contain a valid custom layout JSON.");
    }
  }

  function renderCustomGoogleOauthFields({ compact = false, locked = false } = {}) {
    const hasCustomOauth = hasCustomGoogleOauthSettings(lcstatsSettings);
    return (
      <div
        className={cn(
          compact ? "mt-5" : "",
          locked ? "cursor-not-allowed opacity-45 [&_*]:cursor-not-allowed" : ""
        )}
      >
        <div className="flex items-center justify-between gap-3">
          <button
            type="button"
            className="flex min-w-0 items-center gap-2 text-left text-sm font-semibold text-white/75 transition hover:text-white"
            onClick={() => setCustomGoogleOauthOpen((open) => !open)}
            aria-expanded={customGoogleOauthOpen}
          >
            <ChevronDown
              className={cn(
                "h-4 w-4 shrink-0 text-white/45",
                customGoogleOauthOpen ? "rotate-180" : ""
              )}
            />
            <span>Custom Google OAuth</span>
            <span className="text-xs font-medium text-white/35">
              {hasCustomOauth ? "Override enabled" : "Off, using launcher default"}
            </span>
          </button>

          <a
            href="https://youtu.be/6j4YYn5_mO8"
            target="_blank"
            rel="noreferrer"
            className="group relative inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-full border border-panel-outline bg-black/20 text-white/55 transition hover:bg-white/[0.07] hover:text-white"
            aria-label="Custom Google OAuth setup guide"
          >
            <Info className="h-4 w-4" />
            <span className="pointer-events-none absolute right-0 top-[calc(100%+0.5rem)] z-20 hidden w-56 rounded-lg border border-panel-outline bg-[var(--theme-surface)] px-3 py-2 text-left text-xs font-medium leading-5 text-white/75 shadow-xl group-hover:block">
              Custom Google OAuth setup guide
            </span>
          </a>
        </div>

        <div
          className={cn(
            "oauth-expand mt-3",
            customGoogleOauthOpen ? "oauth-expand-open" : ""
          )}
        >
          <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
            <div className="space-y-2">
              <label className="block text-xs font-semibold text-white/50">
                Client ID
              </label>
              <Input
                value={lcstatsSettings.googleClientId}
                disabled={
                  lcstatsBusy ||
                  lcstatsRefreshBusy ||
                  lcstatsPickerBusy ||
                  googleOauthBusy ||
                  locked
                }
                onChange={(event) =>
                  updateLcstatsSettings({ googleClientId: event.target.value })
                }
                placeholder="Use launcher default"
              />
            </div>
            <div className="space-y-2">
              <label className="block text-xs font-semibold text-white/50">
                Client Secret
              </label>
              <Input
                type="password"
                value={lcstatsSettings.googleClientSecret}
                disabled={
                  lcstatsBusy ||
                  lcstatsRefreshBusy ||
                  lcstatsPickerBusy ||
                  googleOauthBusy ||
                  locked
                }
                onChange={(event) =>
                  updateLcstatsSettings({ googleClientSecret: event.target.value })
                }
                placeholder="Use launcher default"
              />
            </div>
          </div>
        </div>
      </div>
    );
  }

  function renderCustomLcstatsLayoutFields({ locked = false } = {}) {
    if (lcstatsSettings.layout !== "Custom Layout") return null;
    const customLayout = normalizeCustomLcstatsLayout(lcstatsSettings.customLayout);
    const disabled = locked || lcstatsBusy || lcstatsRefreshBusy || lcstatsPickerBusy;
    const sections = [
      {
        title: "Rows",
        columns: "grid-cols-[repeat(auto-fit,minmax(7.25rem,1fr))]",
        groups: [
          [
            ["Start row", "startRow", "3", "number"],
            ["Check column", "checkColumn", "O"],
          ],
          [
            ["Text case", "textCase", "Original", "case"],
            ["Time format", "timeFormat", "7:40 AM", "timeFormat"],
          ],
        ],
      },
      {
        title: "Run",
        columns: "grid-cols-[repeat(auto-fit,minmax(7.25rem,1fr))]",
        groups: [
          [
            ["Quota", "quotaColumn", "B"],
            ["Seed", "seedColumn", ""],
          ],
          [
            ["Moon", "moonColumn", "F"],
            ["Weather", "weatherColumn", "G"],
          ],
          [
            ["Layout", "layoutColumn", "H"],
            ["Item count", "itemCountColumn", "I"],
            ["Apparatus", "apparatusColumn", ""],
            ["App less", "appLessColumn", ""],
          ],
        ],
      },
      {
        title: "Scrap",
        columns: "grid-cols-[repeat(auto-fit,minmax(7.25rem,1fr))]",
        groups: [
          [
            ["Bee amount", "beeAmountColumn", "J"],
            ["Split count", "splitHiveCount", "", "checkbox", "Cheap/exp"],
            ["Bee collected", "beehiveCollectedColumn", ""],
            ["Collected bee value", "beehiveCollectedValueColumn", ""],
            [
              "Collected bee note",
              "beehiveCollectedNotesEnabled",
              "",
              "checkbox",
              "Add note",
            ],
            ["Bee value", "beeValueColumn", "K"],
            ["Cheap hive", "cheapHiveColumn", ""],
            ["Exp hive", "expensiveHiveColumn", ""],
            [
              "hive zero",
              "writeZeroForMissingHives",
              "",
              "checkbox",
              "Write 0",
            ],
          ],
          [
            ["Egg", "eggColumn", "L"],
            ["Egg price note", "eggNotesEnabled", "", "checkbox", "Add note"],
            ["Available egg value", "availableEggValueColumn", ""],
            ["Total outdoor value", "availableOutdoorValueColumn", ""],
          ],
          [
            ["Collected egg value", "collectedEggColumn", ""],
            [
              "Collected egg note",
              "collectedEggNotesEnabled",
              "",
              "checkbox",
              "Add note",
            ],
          ],
          [
            ["Collected", "collectedColumn", "O"],
            ["Available", "availableColumn", "P"],
            ["Real available", "realAvailableColumn", ""],
            ["Collected no extra", "collectedNoExtraColumn", ""],
          ],
          [
            ["Missing", "missingColumn", "Q"],
            [
              "Gift filter",
              "filterCollectedGiftScrapFromMissing",
              "",
              "checkbox",
              "Filter gifts",
            ],
            ["Lost scrap", "lostScrapColumn", "AB"],
            ["Outside items", "outsideItemsColumn", ""],
          ],
          [
            ["Sold", "soldColumn", "X"],
            ["Gifts", "giftsColumn", "AI"],
            ["Gift net only", "giftBoxesNetOnly", "", "checkbox", "Net only"],
          ],
        ],
      },
      {
        title: "Events",
        columns: "grid-cols-[repeat(auto-fit,minmax(7.25rem,1fr))]",
        groups: [
          [
            ["Nutcracker", "nutColumn", "M"],
            ["Nut collect", "nutCollectColumn", ""],
            ["Nut note", "nutNotesEnabled", "", "checkbox", "Add note"],
          ],
          [
            ["Butler", "butlerColumn", "N"],
            ["Butler collect", "butlerCollectColumn", ""],
            ["Butler note", "butlerNotesEnabled", "", "checkbox", "Add note"],
          ],
          [
            ["SID", "sidColumn", "Y"],
            ["SID note", "sidNotesEnabled", "", "checkbox", "Add note"],
            ["SID false", "sidWriteFalse", "", "checkbox", "Write false"],
            ["SID item", "sidItemColumn", ""],
            ["Infes", "infestationColumn", "Z"],
            ["Infes false", "infestationWriteFalse", "", "checkbox", "Write false"],
          ],
          [
            ["Fog", "fogColumn", "AG"],
            ["Fog false", "fogWriteFalse", "", "checkbox", "Write false"],
          ],
          [
            ["Meteor", "meteorColumn", "AH"],
            ["Meteor false", "meteorWriteFalse", "", "checkbox", "Write false"],
          ],
          [
            ["Take off time", "takeoffTimeColumn", ""],
          ],
          [
            ["Turrets", "turretColumn", ""],
            ["Landmines", "landmineColumn", ""],
            ["Spiketraps", "spiketrapColumn", ""],
          ],
        ],
      },
      {
        title: "Enemy",
        columns: "grid-cols-[repeat(auto-fit,minmax(13rem,1fr))]",
        groups: [
          [
            [
              "Death enemy note",
              "deathEnemyNotesEnabled",
              "",
              "checkbox",
              "Add note",
            ],
            ["Write false", "enemyWriteFalse", "", "checkbox", "Write false"],
            ["Write 0", "enemyWriteZero", "", "checkbox", "Write 0"],
          ],
          [
            ["Jester (Jester)", "jesterColumn", ""],
            ["Barber (Clay Surgeon / ClaySurgeon)", "barberColumn", ""],
            ["Bunker Spider (Bunker Spider / SandSpider)", "bunkerSpiderColumn", ""],
            ["Bracken (Flowerman)", "brackenColumn", ""],
          ],
          [
            ["Cadaver (Cadaver Growths)", "cadaverColumn", ""],
            ["Ghost Girl (Girl)", "ghostGirlColumn", ""],
            ["Maneater (Maneater / CaveDweller)", "maneaterColumn", ""],
          ],
          [
            ["Backwater Gunkfish (Stingray)", "backwaterGunkfishColumn", ""],
            ["Coil Head (Spring)", "coilHeadColumn", ""],
            ["Hoarding Bug (Hoarding bug)", "hoardingBugColumn", ""],
            ["Masked (MaskedPlayerEnemy / Masked)", "maskedColumn", ""],
          ],
          [
            ["Snare Flea (Centipede)", "snareFleaColumn", ""],
            ["Spore Lizard (Puffer)", "sporeLizardColumn", ""],
            ["Thumper (Crawler)", "thumperColumn", ""],
          ],
          [
            ["Earth Leviathan", "earthLeviathanColumn", ""],
            ["Forest Giant (ForestGiant)", "forestGiantColumn", ""],
            ["Baboon Hawk", "baboonHawkColumn", ""],
          ],
          [
            ["Old Bird (RadMech)", "oldBirdColumn", ""],
            ["Bush Wolf", "bushWolfColumn", ""],
            ["Feiopar", "feioparColumn", ""],
          ],
          [
            ["Eyeless Dog (MouthDog)", "eyelessDogColumn", ""],
          ],
        ],
      },
      {
        title: "Players",
        columns: "grid-cols-[repeat(auto-fit,minmax(13rem,1fr))]",
        groups: [
          [
            ["Death state columns", "deathColumns", "AC,AD,AE,AF", "list"],
            ["Player name columns", "playerNameColumns", "AB,AC,AD,AE", "list"],
          ],
          [
            ["Player name row", "playerNameRow", "1", "number"],
          ],
          [
            ["Alive value", "aliveState", "S", "text"],
            ["Dead value", "deadState", "X", "text"],
            ["Late death", "lateDeadState", "SX", "text"],
            ["Missing value", "missingState", "M", "text"],
            ["Disconnected value", "disconnectedState", "DC", "text"],
            ["Death reason notes", "deathNotesEnabled", "", "checkbox", "Use notes"],
            ["Names as notes", "playerNamesAsNotes", "", "checkbox", "Use notes"],
          ],
        ],
      },
    ];
    const customLayoutSearchQuery = customLayoutSearch.trim().toLowerCase();
    const fieldMatchesSearch = (sectionTitle, field) => {
      if (!customLayoutSearchQuery) return true;
      const [label, key, placeholder, type, checkboxText] = field;
      return [sectionTitle, label, key, placeholder, type, checkboxText]
        .filter(Boolean)
        .some((value) => String(value).toLowerCase().includes(customLayoutSearchQuery));
    };
    const visibleSections = sections
      .map((section) => {
        if (!customLayoutSearchQuery) return section;
        const sectionTitleMatches = section.title
          .toLowerCase()
          .includes(customLayoutSearchQuery);
        if (sectionTitleMatches) return section;
        const groups = (section.groups ?? [section.fields ?? []])
          .map((group) =>
            group.filter((field) => fieldMatchesSearch(section.title, field))
          )
          .filter((group) => group.length > 0);
        return groups.length > 0 ? { ...section, groups } : null;
      })
      .filter(Boolean);

    return (
      <div className="space-y-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="text-xs font-semibold text-white/50">Custom columns</div>
          <div className="flex items-center gap-2">
            <Button
              variant="secondary"
              className="h-8 px-3 text-xs"
              onClick={() => {
                openCustomLayoutDocs().catch(console.error);
              }}
            >
              <Info className="h-3.5 w-3.5" />
              Docs
            </Button>
            <Button
              variant="secondary"
              className="h-8 px-3 text-xs"
              disabled={disabled}
              onClick={() => {
                updateLcstatsSettings({
                  customLayout: normalizeCustomLcstatsLayout(
                    DEFAULT_CUSTOM_LCSTATS_LAYOUT
                  ),
                });
                setCustomLayoutMessage("Custom layout reset.");
                setCustomLayoutMessageKind("success");
              }}
            >
              Reset
            </Button>
            <Button
              variant="secondary"
              className="h-8 px-3 text-xs"
              disabled={disabled}
              onClick={() => copyCustomLcstatsLayout().catch(console.error)}
            >
              Copy
            </Button>
            <Button
              variant="secondary"
              className="h-8 px-3 text-xs"
              disabled={disabled || !customLayoutClipboardValid}
              onMouseEnter={() => refreshCustomLayoutClipboardStatus().catch(() => {})}
              onFocus={() => refreshCustomLayoutClipboardStatus().catch(() => {})}
              onClick={() => loadCustomLcstatsLayoutFromClipboard().catch(console.error)}
            >
              Load
            </Button>
          </div>
        </div>
        <div className="relative">
          <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-white/40" />
          <Input
            value={customLayoutSearch}
            disabled={disabled}
            onChange={(event) => setCustomLayoutSearch(event.target.value)}
            placeholder="Search custom settings"
            className="pl-10 pr-9"
          />
          {customLayoutSearch ? (
            <button
              type="button"
              className="absolute right-2 top-1/2 flex h-6 w-6 -translate-y-1/2 items-center justify-center rounded-full text-white/40 transition hover:bg-white/[0.07] hover:text-white"
              onClick={() => setCustomLayoutSearch("")}
              aria-label="Clear custom settings search"
            >
              <X className="h-4 w-4" />
            </button>
          ) : null}
        </div>
        {customLayoutMessage ? (
          <div
            className={cn(
              "rounded-xl border px-3 py-2 text-xs",
              customLayoutMessageKind === "error"
                ? "border-red-400/30 bg-red-400/10 text-red-200"
                : "border-[color-mix(in_srgb,var(--theme-accent)_35%,transparent)] bg-[var(--theme-accent-muted)] text-[var(--theme-accent)]"
            )}
          >
            {customLayoutMessage}
          </div>
        ) : null}
        {visibleSections.length === 0 ? (
          <div className="rounded-xl border border-panel-outline bg-white/[0.03] px-3 py-3 text-sm text-white/45">
            No custom settings found.
          </div>
        ) : null}
        {visibleSections.map((section) => {
          const open = customLayoutSectionOpen[section.title] ?? true;
          const visibleOpen = customLayoutSearchQuery ? true : open;
          const groups = section.groups ?? [section.fields ?? []];
          const fieldCount = groups.reduce((count, group) => count + group.length, 0);
          return (
            <div key={section.title}>
              <button
                type="button"
                className="flex min-w-0 items-center gap-2 text-left text-sm font-semibold text-white/75 transition hover:text-white"
                onClick={() =>
                  setCustomLayoutSectionOpen((prev) => ({
                    ...prev,
                    [section.title]: !(prev[section.title] ?? true),
                  }))
                }
                aria-expanded={open}
              >
                <ChevronDown
                  className={cn(
                    "h-4 w-4 shrink-0 text-white/45 transition-transform",
                    visibleOpen ? "rotate-180" : ""
                  )}
                />
                <span>{section.title}</span>
                <span className="text-xs font-medium text-white/35">
                  {fieldCount} fields
                </span>
              </button>
              <div
                className={cn(
                  "oauth-expand custom-layout-expand mt-3",
                  visibleOpen ? "oauth-expand-open" : ""
                )}
              >
                <div className="space-y-3">
                  {groups.map((group, groupIndex) => (
                    <div
                      key={`${section.title}-${groupIndex}`}
                      className={cn("grid grid-cols-1 gap-3", section.columns)}
                    >
                      {group.map(([label, key, placeholder, type, checkboxText]) => (
                        <div key={key} className="min-w-0 space-y-2">
                          <label className="block text-xs font-semibold text-white/50">
                            {label}
                          </label>
                          {type === "checkbox" ? (
                            <label className="flex h-10 min-w-0 items-center gap-2 overflow-hidden rounded-xl border border-panel-outline bg-black/20 px-3 text-sm text-white/70">
                              <Checkbox
                                checked={!!customLayout[key]}
                                disabled={disabled}
                                onCheckedChange={(checked) =>
                                  updateCustomLcstatsLayout({ [key]: !!checked })
                                }
                              />
                              <span className="min-w-0 truncate whitespace-nowrap">
                                {checkboxText ?? "Enabled"}
                              </span>
                            </label>
                          ) : type === "case" || type === "timeFormat" ? (
                            <Select
                              value={customLayout[key]}
                              onValueChange={(value) =>
                                updateCustomLcstatsLayout({ [key]: value })
                              }
                              disabled={disabled}
                            >
                              <SelectTrigger>
                                <SelectValue placeholder={placeholder} />
                              </SelectTrigger>
                              <SelectContent>
                                {(type === "case"
                                  ? [
                                      "Original",
                                      "UPPERCASE",
                                      "lowercase",
                                      "Title Case",
                                      "camelCase",
                                      "PascalCase",
                                    ]
                                  : [
                                      ...CUSTOM_TIME_FORMAT_OPTIONS,
                                    ]
                                ).map((option) => (
                                  <SelectItem
                                    key={typeof option === "string" ? option : option.value}
                                    value={typeof option === "string" ? option : option.value}
                                  >
                                    {typeof option === "string" ? option : option.label}
                                  </SelectItem>
                                ))}
                              </SelectContent>
                            </Select>
                          ) : (
                            <Input
                              type={type === "number" ? "number" : "text"}
                              min={type === "number" ? 1 : undefined}
                              value={customLayout[key]}
                              disabled={disabled}
                              onChange={(event) => {
                                const value = event.target.value;
                                updateCustomLcstatsLayout({
                                  [key]:
                                    type === "number"
                                      ? Math.max(1, Math.floor(Number(value) || 1))
                                      : type === "list"
                                        ? normalizeSheetColumnList(value, "")
                                        : type === "text"
                                          ? value
                                          : normalizeSheetColumn(value, ""),
                                });
                              }}
                              placeholder={placeholder}
                            />
                          )}
                        </div>
                      ))}
                    </div>
                  ))}
                </div>
              </div>
            </div>
          );
        })}
      </div>
    );
  }

  async function resetLcstatsSheet() {
    if (lcstatsResetInFlight.current) return;
    lcstatsResetInFlight.current = true;
    setLcstatsResetBusy(true);
    setLcstatsError("");
    setLcstatsSaved("");
    try {
      await invoke("reset_lcstats_sheet", {
        settings: normalizeLcstatsSettings(lcstatsSettings),
        endRow: lcstatsSettings.layout === "WafrodyAutoSheet" ? 93 : Number(lcstatsResetEndRow),
      });
      setLcstatsSaved("Reset complete.");
    } catch (e) {
      setLcstatsError(e?.message ?? String(e));
    } finally {
      lcstatsResetInFlight.current = false;
      setLcstatsResetBusy(false);
    }
  }

  function renderLcStatsSettingsPanel() {
    const spreadsheetId = extractSpreadsheetId(lcstatsSettings.spreadsheetId);
    const listedSpreadsheet = lcstatsSpreadsheets.find(
      (file) => file.id === spreadsheetId
    );
    const spreadsheetName = listedSpreadsheet?.name || lcstatsSpreadsheetName;
    const spreadsheetDisplayValue = lcstatsSpreadsheetFocused
      ? lcstatsSettings.spreadsheetId
      : spreadsheetName || lcstatsSettings.spreadsheetId;
    const useSpreadsheetPicker = !hasCustomGoogleOauthSettings(lcstatsSettings);
    const settingsLocked = modEnabled && !googleOauthStatus.authenticated;
    const settingsDisabled =
      settingsLocked || lcstatsBusy || lcstatsRefreshBusy || lcstatsPickerBusy || lcstatsResetBusy;
    const googleButtonLabel = googleOauthStatus.authenticated
      ? "Google Logout"
      : "Google Login";
    const handleGoogleOauthButtonClick = () => {
      if (googleOauthStatus.authenticated) {
        logoutGoogleOauth().catch(console.error);
      } else {
        setGoogleOauthError("");
        setGoogleOauthDialogOpen(true);
      }
    };

    return (
      <div className="min-h-0 flex flex-1 flex-col gap-3 overflow-hidden">
        <div className="grid shrink-0 grid-cols-1 gap-2">
          <Button
            variant={googleOauthStatus.authenticated ? "secondary" : "default"}
            className="h-10 w-full"
            disabled={googleOauthBusy}
            onClick={handleGoogleOauthButtonClick}
          >
            {googleButtonLabel}
          </Button>
          <div className="grid grid-cols-2 gap-2">
          <Button
            variant="secondary"
            className="h-10 w-full"
            disabled={lcstatsResetBusy || lcstatsTrackingBusy || !selectedLcStatsTracker || !lcstatsSettings.useLcstatsApi}
            onClick={() => {
              toggleLcstatsAutosheetTracking().catch(console.error);
            }}
          >
            {lcstatsTrackingBusy ? (
              <LoaderCircle className="h-4 w-4 animate-spin" />
            ) : lcstatsTrackingEnabled ? (
              <X className="h-4 w-4" />
            ) : (
              <CheckCircle2 className="h-4 w-4" />
            )}
            {!lcstatsSettings.useLcstatsApi
              ? "LCStats API Disabled"
              : lcstatsTrackingEnabled
                ? "Stop Tracking"
                : "Track Current Game"}
          </Button>
              <Button
                variant="secondary"
                className="h-10 w-full"
                title="Reset the selected sheet"
                disabled={settingsDisabled || lcstatsTrackingBusy || lcstatsTrackingEnabled || !googleOauthStatus.authenticated || !spreadsheetId || !lcstatsSettings.activeSheetName}
                onClick={resetLcstatsSheet}
              >{lcstatsResetBusy ? "Resetting…" : "Reset Sheet"}</Button>
          </div>
        </div>
        <div className="min-h-0 flex-1 overflow-auto rounded-2xl border border-panel-outline bg-[var(--theme-surface)] p-4">
          <div className="space-y-5">
            {renderCustomGoogleOauthFields()}

            {googleOauthError ? (
              <div className="rounded-2xl border border-red-400/30 bg-red-400/10 px-4 py-3 text-sm text-red-200">
                {googleOauthError}
              </div>
            ) : null}

            <div className="flex items-center justify-between gap-4 rounded-2xl border border-panel-outline bg-white/[0.04] px-4 py-3">
              <div>
                <div className="text-sm font-medium text-white">
                  Use LCStatsTracker API
                </div>
                <div className="mt-1 text-xs text-white/50">
                  Lets the launcher listen to LCStatsTracker for AutoSheet and HQLC overlay data.
                </div>
              </div>
              <Switch
                checked={lcstatsSettings.useLcstatsApi !== false}
                disabled={lcstatsBusy}
                onCheckedChange={(checked) => {
                  updateLcstatsSettings({ useLcstatsApi: checked });
                }}
              />
            </div>

            {!lcstatsSettings.useLcstatsApi ? (
              <div className="rounded-2xl border border-yellow-300/25 bg-yellow-300/10 px-4 py-3 text-sm leading-relaxed text-yellow-100">
                LCStatsTracker API access is disabled. The mod can still run, but this launcher will not connect to its local stats feed.
              </div>
            ) : null}

            <div
              className={cn(
                "space-y-5 transition-opacity",
                settingsLocked ? "cursor-not-allowed opacity-45 [&_*]:cursor-not-allowed" : ""
              )}
            >
              <div
                className={cn(
                  "grid grid-cols-1 gap-3",
                  "md:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_2.5rem]"
                )}
              >
                <div className="min-w-0 space-y-2">
                  <label className="block text-xs font-semibold text-white/50">
                    Spreadsheet
                  </label>
                  {useSpreadsheetPicker ? (
                    <button
                      type="button"
                      className={cn(
                        "flex h-10 w-full items-center justify-between gap-3 rounded-xl border border-panel-outline bg-black/20 px-4 text-left text-sm text-white outline-none transition hover:bg-white/[0.07] focus:border-panel-outline focus:ring-2 focus:ring-panel-outline disabled:cursor-not-allowed disabled:bg-black/10 disabled:text-white/45 disabled:opacity-60",
                        !spreadsheetDisplayValue ? "text-white/40" : ""
                      )}
                      disabled={settingsDisabled}
                      onClick={() => {
                        openLcstatsSpreadsheetPicker().catch(console.error);
                      }}
                      title={spreadsheetName && spreadsheetId ? spreadsheetId : undefined}
                    >
                      <span className="min-w-0 truncate">
                        {spreadsheetDisplayValue || "Select spreadsheet"}
                      </span>
                      {lcstatsPickerBusy ? (
                        <LoaderCircle className="h-4 w-4 shrink-0 animate-spin text-white/55" />
                      ) : (
                        <FolderOpen className="h-4 w-4 shrink-0 text-white/55" />
                      )}
                    </button>
                  ) : (
                    <Input
                      value={spreadsheetDisplayValue}
                      disabled={settingsDisabled}
                      onFocus={(event) => {
                        setLcstatsSpreadsheetFocused(true);
                        window.requestAnimationFrame(() => event.target.select());
                      }}
                      onChange={(event) =>
                        handleLcstatsSpreadsheetInputChange(event.target.value)
                      }
                      onBlur={(event) => {
                        setLcstatsSpreadsheetFocused(false);
                        loadLcstatsSheetNamesFromParsedInput(
                          parseSpreadsheetInput(event.target.value)
                        );
                      }}
                      placeholder="Google Sheets link or spreadsheet ID"
                      title={spreadsheetName && spreadsheetId ? spreadsheetId : undefined}
                    />
                  )}
                </div>
                <div className="min-w-0 space-y-2">
                  <label className="block text-xs font-semibold text-white/50">
                    Sheet
                  </label>
                  {lcstatsSheets.length > 0 ? (
                    <Select
                      value={lcstatsSettings.activeSheetName}
                      onValueChange={(value) => {
                        const sheet = findSheetInfoByTitle(lcstatsSheetInfos, value);
                        updateLcstatsSettings({
                          activeSheetName: value,
                          activeSheetId: sheet?.sheetId ?? "",
                        });
                      }}
                      disabled={settingsDisabled}
                    >
                      <SelectTrigger>
                        <SelectValue placeholder="Select sheet" />
                      </SelectTrigger>
                      <SelectContent>
                        {lcstatsSheets.map((name) => (
                          <SelectItem key={name} value={name}>
                            {name}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  ) : (
                    <Input
                      value={lcstatsSettings.activeSheetName}
                      disabled={settingsDisabled}
                      onChange={(event) =>
                        updateLcstatsSettings({ activeSheetName: event.target.value })
                      }
                      placeholder="Sheet1"
                    />
                  )}
                </div>
                <div className="flex items-end">
                  <Button
                    variant="secondary"
                    className="h-10 w-10 shrink-0 px-0 disabled:cursor-not-allowed disabled:pointer-events-auto"
                    disabled={settingsDisabled}
                    onClick={() => {
                      refreshLcstatsGoogleLists().catch(console.error);
                    }}
                    title="Refresh Google Sheets"
                    aria-label="Refresh Google Sheets"
                  >
                    <RefreshCw
                      className={cn(
                        "h-4 w-4",
                        lcstatsRefreshBusy ? "animate-spin" : ""
                      )}
                    />
                  </Button>
                </div>
              </div>

              <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
                <div className="space-y-2">
                  <label className="block text-xs font-semibold text-white/50">
                    Layout
                  </label>
                  <Select
                    value={lcstatsSettings.layout}
                    onValueChange={(value) =>
                      updateLcstatsSettings(
                        lcstatsLayoutUsesColumnFields(value)
                          ? {
                              layout: value,
                              startColumn: lcstatsSettings.startColumn || "D",
                              quotaColumn: lcstatsSettings.quotaColumn || "B",
                              sellColumn: lcstatsSettings.sellColumn || "AE",
                            }
                          : {
                              layout: value,
                              startColumn: "",
                              quotaColumn: "",
                              sellColumn: "",
                            }
                      )
                    }
                    disabled={settingsDisabled}
                  >
                    <SelectTrigger>
                      <SelectValue placeholder="Select layout" />
                    </SelectTrigger>
                    <SelectContent>
                      {LCSTATS_LAYOUTS.map((layout) => (
                        <SelectItem key={layout} value={layout}>
                          {layout}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
              </div>

              {lcstatsLayoutUsesColumnFields(lcstatsSettings.layout) ? (
              <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
                {[
                  ["Start column", "startColumn", "D"],
                  ["Quota column", "quotaColumn", "B"],
                  ["Sell column", "sellColumn", "AE"],
                ].map(([label, key, placeholder]) => (
                  <div key={key} className="space-y-2">
                    <label className="block text-xs font-semibold text-white/50">
                      {label}
                    </label>
                    <Input
                      value={lcstatsSettings[key]}
                      disabled={settingsDisabled}
                      onChange={(event) =>
                        updateLcstatsSettings({
                          [key]: normalizeSheetColumn(event.target.value, ""),
                        })
                      }
                      placeholder={placeholder}
                    />
                  </div>
                ))}
              </div>
              ) : null}

              {lcstatsSettings.layout !== "WafrodyAutoSheet" ? (
                <label className="block space-y-2 text-xs text-white/60">
                  Reset last data row (inclusive)
                  <Input type="number" min="1" max="10000" value={lcstatsResetEndRow}
                    disabled={settingsDisabled}
                    onChange={(event) => setLcstatsResetEndRow(event.target.value)} />
                </label>
              ) : null}

              {renderCustomLcstatsLayoutFields({ locked: settingsLocked })}
            </div>

            {lcstatsError ? (
              <div className="rounded-2xl border border-red-400/30 bg-red-400/10 px-4 py-3 text-sm text-red-200">
                {lcstatsError}
              </div>
            ) : null}

            {lcstatsSaved ? (
              <div className="rounded-2xl border border-[color-mix(in_srgb,var(--theme-accent)_35%,transparent)] bg-[var(--theme-accent-muted)] px-4 py-3 text-sm text-[var(--theme-accent)]">
                {lcstatsSaved}
              </div>
            ) : null}

          </div>
        </div>
      </div>
    );
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
    if (isEclipsedRunMode(runMode) && !nextEnabled && eclipsedForcedModKeys.has(baseKey)) {
      return;
    }
    if (
      nextEnabled &&
      !opts?.skipGoogleOauthGate &&
      requiresGoogleOauthForMod(mod) &&
      !lcstatsSettings.allowWithoutGoogle
    ) {
      const status = googleOauthStatus.authenticated
        ? googleOauthStatus
        : await refreshGoogleOauthStatus().catch(() => googleOauthStatus);
      if (!status.authenticated) {
        requestGoogleOauthForToggle(mod, nextEnabled, opts);
        return;
      }
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
            allowWithoutGoogle: !!opts?.allowWithoutGoogle,
          })
        )
      );
      // refresh disabled list (source of truth)
      const dm = await invoke("get_disabled_mods");
      setDisabledMods(Array.isArray(dm) ? dm : []);
      await refreshInstalledModVersions(selectedVersion, {
        retries: nextEnabled ? 3 : 0,
        delayMs: 180,
        expectedKeys: nextEnabled ? toggledKeys : [],
      });
      if (affectsSelected) setModEnabled(!!nextEnabled);
      return true;
    } catch (e) {
      console.error(e);
      setCfgError(e?.message ?? String(e));
      return false;
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

  async function switchModVariant(currentMod, targetMod) {
    if (!currentMod || !targetMod) return;
    await toggleModEnabledForMod(targetMod, true, {
      propagateChain: false,
    });
  }

  async function refreshLcstatsAutosheetTracking() {
    const running = await invoke("get_lcstats_autosheet_tracking");
    setLcstatsTrackingEnabled(!!running);
    return !!running;
  }

  async function toggleLcstatsAutosheetTracking() {
    if (!selectedLcStatsTracker) return;

    const nextEnabled = !lcstatsTrackingEnabled;
    setLcstatsTrackingBusy(true);
    setLcstatsError("");

    try {
      const running = await invoke("set_lcstats_autosheet_tracking", {
        enabled: nextEnabled,
      });
      setLcstatsTrackingEnabled(!!running);
    } catch (e) {
      console.error(e);
      setLcstatsError(e?.message ?? String(e));
      if (nextEnabled && String(e?.message ?? e).includes("Google login")) {
        setGoogleOauthError("");
        setGoogleOauthDialogOpen(true);
      }
    } finally {
      setLcstatsTrackingBusy(false);
    }
  }

  async function openCustomLayoutDocs() {
    setLcstatsError("");
    try {
      await invoke("open_custom_layout_docs");
    } catch (e) {
      console.error(e);
      setLcstatsError(e?.message ?? String(e));
    }
  }

  async function resetFreeMoonsMod(version) {
    const v = Number(version);
    if (!Number.isFinite(v)) return;

    const mods = (Array.isArray(manifest.mods) ? manifest.mods : []).filter((m) =>
      ECLIPSED_HQ_OPTIONAL_MOD_KEYS.has(modKeyLower(m))
    );
    if (mods.length === 0) return;

    try {
      await Promise.all(
        mods.map((m) =>
          invoke("set_mod_enabled", {
            version: v,
            dev: m.dev,
            name: m.name,
            enabled: false,
          })
        )
      );
      const dm = await invoke("get_disabled_mods");
      setDisabledMods(Array.isArray(dm) ? dm : []);
    } catch (e) {
      console.error(e);
    }
  }

  const selectedRunModeRange = useMemo(
    () => getPresetVersionRange(manifest, selectedEvent ? selectedEventRunMode : runMode),
    [manifest, runMode, selectedEvent, selectedEventRunMode]
  );

  const versionOptions = useMemo(() => {
    const set = new Set(installedVersions);
    set.add(selectedVersion);
    if (selectedRunModeRange?.low != null) set.add(selectedRunModeRange.low);
    if (selectedRunModeRange?.high != null) set.add(selectedRunModeRange.high);
    if (selectedEvent && Array.isArray(selectedEvent.versions)) {
      selectedEvent.versions
        .map((v) => Number(v))
        .filter((v) => Number.isFinite(v))
        .forEach((v) => set.add(v));
    }
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
    if (selectedEvent) {
      list = list.filter((v) => eventAllowsVersion(selectedEvent, v));
    }
    return list;
  }, [installedVersions, selectedVersion, manifest, selectedRunModeRange, selectedEvent]);

  // If the selected version is outside the allowed range, bump it back in range.
  useEffect(() => {
    if (!Number.isFinite(Number(selectedVersion))) return;
    if (
      isVersionWithinRange(selectedVersion, selectedRunModeRange) &&
      (!selectedEvent || eventAllowsVersion(selectedEvent, selectedVersion))
    ) {
      return;
    }

    const rangeClamped = clampVersionToRange(selectedVersion, selectedRunModeRange);
    const nextV = selectedEvent
      ? clampVersionToEvent(rangeClamped, selectedEvent, installedVersions)
      : rangeClamped;
    const shouldRunSideEffects = didFinishBootstrap;
    setSelectedVersion(nextV);
    if (!shouldRunSideEffects) return;
    if (isInstalled(nextV)) {
      invoke("apply_disabled_mods", { version: nextV })
        .catch(() => {})
        .finally(() => {
          prepareRunMode(selectedEvent ? selectedEventRunMode : runMode, nextV, {
            eventId: selectedEvent?.id,
          }).catch(() => {});
        });
    } else {
      openDownloadPrompt(nextV);
    }
  }, [
    didFinishBootstrap,
    selectedRunModeRange,
    selectedVersion,
    installedVersions,
    runMode,
    selectedEvent,
    selectedEventRunMode,
  ]);

  const selectedInstalled = isInstalled(selectedVersion);
  const selectedVersionLabel = Number.isFinite(Number(selectedVersion))
    ? `v${selectedVersion}`
    : "Select version";
  const selectedPresetSummary = isPresetSummaryMod(selectedMod) ? selectedMod : null;
  const selectedLcStatsTracker = isLcStatsTrackerMod(selectedMod) ? selectedMod : null;
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

  function openLaunchContextMenu(event) {
    event.preventDefault();
    event.stopPropagation();
    const version = Number(selectedVersion);
    if (!Number.isFinite(version) || !isInstalled(version)) return;
    setLaunchContextMenu({
      open: true,
      x: event.clientX,
      y: event.clientY,
    });
  }

  function closeLaunchContextMenu() {
    setLaunchContextMenu((prev) => ({ ...prev, open: false }));
  }

  async function openModContextMenu(event, mod) {
    event.preventDefault();
    const version = Number(selectedVersion);
    const keyLower = `${String(mod?.dev ?? "").toLowerCase()}::${String(
      mod?.name ?? ""
    ).toLowerCase()}`;
    let configPath = "";

    const selectedKeyLower = selectedMod ? modKeyLower(selectedMod) : "";
    if (
      selectedKeyLower === keyLower &&
      String(activeConfigPath ?? "").toLowerCase().endsWith(".cfg")
    ) {
      configPath = String(activeConfigPath);
    }

    const cachedFiles = modCfgFilesByKey[`${version}::${keyLower}`];
    if (!configPath && Array.isArray(cachedFiles)) {
      configPath = cachedFiles.find((p) =>
        String(p).toLowerCase().endsWith(".cfg")
      ) ?? "";
    }
    if (!configPath && Number.isFinite(version) && mod?.dev && mod?.name) {
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

  async function openSelectedModThunderstore(mod) {
    if (!mod?.dev || !mod?.name) return;
    const url = `https://thunderstore.io/c/lethal-company/p/${encodeURIComponent(
      mod.dev
    )}/${encodeURIComponent(mod.name)}`;
    try {
      await openUrl(url);
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
  const updateIsStorageMove = updatePrompt.open && task.step_name === "Move Storage";


  const selectedRunOption = useMemo(() => {
    return (
      RUN_OPTIONS.find((o) => o.value === runMode) ??
      RUN_OPTIONS.find((o) => o.value === "hq") ??
      RUN_OPTIONS[0]
    );
  }, [runMode]);

  const selectedLaunchLabel = selectedEvent?.name
    ? selectedEvent.name
    : selectedRunOption?.buttonLabel ?? selectedRunOption?.label ?? "Start Run";
  const selectedLaunchTitle = selectedEvent?.name
    ? `${selectedEvent.name} (${selectedRunOption?.label ?? "HQ Run"})`
    : selectedRunOption?.title ?? "";

  const discordRunLabel = useMemo(() => {
    const value = selectedRunOption?.value;
    if (!value) return "High Quota Run";
    if (value === "hq") return "High Quota Run";
    if (value === "vanilla") return "Vanilla Run";
    if (value === "practice") return "High Quota Practice";
    if (value === "c_moons") return "Classic Moons Run";
    if (value === "c_moons_smhq") return "Classic Moons SMHQ";
    if (value === "c_moons_eclipsed") return "Classic Moons Eclipsed";
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
    if (value === "c_moons_eclipsed") return "Classic Moons Eclipsed";
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

    const eventId = String(opts?.eventId ?? "").trim();
    const key = eventId
      ? `event:${eventId}:${nextRunMode}:${nextVersion}`
      : `${nextRunMode}:${nextVersion}`;
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
      if (opt?.vanilla) {
        await refreshInstalledModVersions(nextVersion);
        await refreshConfigLinkState(nextVersion);
        setPreparedUpdateContext(key);
        return;
      }
      if (eventId) {
        await invoke("prepare_event", {
          version: nextVersion,
          eventId,
        });
      } else {
        await invoke("prepare_preset", {
          version: nextVersion,
          preset: opt?.preset ?? "hq",
          practice: !!opt?.practice,
        });
      }

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
      if (eventId) {
        for (const mod of selectedEventMods) {
          expectedPracticeKeys.push(modKeyLower(mod));
        }
      }

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
            saveSelectedRunMode(prev.prevRunMode);
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
      setRunningGames([]);
    } finally {
      setGameStatus({ running: false, pid: null });
      setLcstatsTrackingEnabled(false);
    }
  }

  async function refreshRunningGames() {
    const games = await invoke("list_running_games");
    const list = Array.isArray(games) ? games : [];
    setRunningGames(list);
    setGameStatus({
      running: list.length > 0,
      pid: typeof list[0]?.pid === "number" ? list[0].pid : null,
    });
    if (list.length === 0) {
      setLcstatsTrackingEnabled(false);
    }
    return list;
  }

  async function openRunningGamesDialog() {
    try {
      await refreshRunningGames();
    } catch (e) {
      console.error(e);
    }
    setRunningGamesDialogOpen(true);
  }

  async function stopRunningGame(id) {
    if (!Number.isFinite(Number(id))) return;
    setRunningGameStopBusyId(id);
    try {
      await invoke("stop_game_instance", { id });
      const list = await refreshRunningGames();
      if (list.length === 0) {
        setRunningGamesDialogOpen(false);
      }
    } catch (e) {
      console.error(e);
    } finally {
      setRunningGameStopBusyId(null);
    }
  }

  function updateLaunchOptionEntry(index, value) {
    setLaunchOptionsEntries((prev) => {
      const next = [...prev];
      next[index] = value;
      return next;
    });
  }

  function removeLaunchOptionEntry(index) {
    setLaunchOptionsEntries((prev) => prev.filter((_, entryIndex) => entryIndex !== index));
  }

  function addLaunchOptionEntry() {
    const nextEntry = String(newLaunchOptionEntry ?? "").trim();
    if (!nextEntry) return;
    setLaunchOptionsEntries((prev) => [...prev, nextEntry]);
    setNewLaunchOptionEntry("");
  }

  function getActiveLaunchOptions() {
    if (!launchOptionsEnabled) return [];
    return normalizeLaunchOptionsEntries(launchOptionsEntries);
  }

  function getActiveLaunchCommandTemplate() {
    if (!launchOptionsEnabled) return null;
    const normalized = normalizeLaunchCommandTemplate(launchCommandTemplate);
    return normalized || null;
  }

  async function startSelectedRun(opts = {}) {
    if (launchBusy) return;
    const allowMultiple = opts?.allowMultiple === true;
    if (gameStatus.running && !allowMultiple) return stopRun();

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
      const eventLaunchArgs =
        selectedEvent &&
        (launchRequest.command === "launch_game_preset" ||
          launchRequest.command === "launch_game") &&
        nextRunMode === selectedEventRunMode
          ? { eventId: selectedEvent.id }
          : {};
      const pid = await invoke(launchRequest.command, {
        ...launchRequest.args,
        ...eventLaunchArgs,
        launchOptions: getActiveLaunchOptions(),
        launchCommandTemplate: getActiveLaunchCommandTemplate(),
        allowMultiple,
      });
      setGameStatus({
        running: true,
        pid: typeof pid === "number" ? pid : null,
      });
      refreshLcstatsAutosheetTracking().catch((error) => {
        console.error(error);
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
    prepareRunMode(selectedEvent ? selectedEventRunMode : runMode, version, {
      eventId: selectedEvent?.id,
    }).catch((e) => {
      console.error(e);
      didAutoPrepareInitialRef.current = false;
    });
  }, [
    didFinishBootstrap,
    gameStatus.running,
    isInstalled,
    selectedRunModeRange,
    runMode,
    selectedEvent,
    selectedEventRunMode,
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

  // Save selectedVersion to localStorage and app config.
  useEffect(() => {
    saveSelectedVersion(selectedVersion);
  }, [selectedVersion]);

  useEffect(() => {
    if (
      didFinishBootstrap &&
      didLoadPersistedRunMode &&
      (hasPersistedRunMode || userSelectedRunModeRef.current)
    ) {
      saveSelectedRunMode(runMode);
    }
  }, [didFinishBootstrap, didLoadPersistedRunMode, hasPersistedRunMode, runMode]);

  async function handleRunModeSelect(nextRunMode) {
    userSelectedRunModeRef.current = true;
    const prevRun = runMode;
    const prevVer = selectedVersion;
    const previousEventId = selectedEventId;
    const range = getPresetVersionRange(manifest, nextRunMode);
    const effectiveV = clampVersionToRange(selectedVersion, range);
    const shouldResetFreeMoonsMod =
      prevRun !== nextRunMode &&
      (shouldResetFreeMoonsOnRunModeChange(prevRun) ||
        shouldResetFreeMoonsOnRunModeChange(nextRunMode));

    if (previousEventId) {
      setSelectedEventId("");
      saveSelectedEventId("");
      if (Number.isFinite(Number(selectedVersion))) {
        try {
          await invoke("clear_selected_event", { version: Number(selectedVersion) });
          const dm = await invoke("get_disabled_mods");
          setDisabledMods(Array.isArray(dm) ? dm : []);
        } catch (e) {
          console.error(e);
        }
      }
    }
    setRunMode(nextRunMode);
    saveSelectedRunMode(nextRunMode);
    if (shouldResetFreeMoonsMod) {
      await resetFreeMoonsMod(effectiveV);
    }
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

  async function handleEventSelect(event) {
    if (!event) return;
    userSelectedRunModeRef.current = true;
    const prevRun = runMode;
    const prevVer = selectedVersion;
    const nextRunMode = normalizeEventPreset(event.preset);
    const range = getPresetVersionRange(manifest, nextRunMode);
    const rangeClamped = clampVersionToRange(selectedVersion, range);
    const effectiveV = clampVersionToEvent(rangeClamped, event, installedVersions);

    setSelectedEventId(event.id);
    saveSelectedEventId(event.id);
    setRunMode(nextRunMode);
    saveSelectedRunMode(nextRunMode);

    if (effectiveV !== selectedVersion) {
      setSelectedVersion(effectiveV);
      if (!isInstalled(effectiveV)) {
        openDownloadPrompt(effectiveV);
        return;
      }
      try {
        await invoke("apply_disabled_mods", { version: effectiveV });
      } catch {}
    }

    await prepareRunMode(nextRunMode, effectiveV, {
      prevRunMode: prevRun,
      prevVersion: prevVer,
      eventId: event.id,
    });
  }

  const showBootstrapSkeleton = !didFinishBootstrap;
  const activeRunModeForActions = selectedEvent ? selectedEventRunMode : runMode;
  const checkUpdateMatchesSelectedRun =
    Number(checkUpdateTask.version) === Number(selectedVersion) &&
    checkUpdateTask.run_mode === activeRunModeForActions;
  const selectedRunUpdatableMods = checkUpdateMatchesSelectedRun
    ? Array.isArray(checkUpdateTask.updatable_mods)
      ? checkUpdateTask.updatable_mods
      : []
    : [];
  const hasNativeOverlayUpdate =
    nativeOverlayUpdate?.supported !== false &&
    nativeOverlayUpdate?.available === true;
  const hasSelectedVersionUpdates =
    checkUpdateTask.status === "done" &&
    checkUpdateMatchesSelectedRun &&
    (selectedRunUpdatableMods.length > 0 || hasNativeOverlayUpdate);

  return (
    <div className="h-full bg-[var(--theme-bg)] text-white">
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
              className="h-11 border-panel-outline bg-[var(--theme-surface)] px-5 text-white hover:bg-white/[0.07]"
              onClick={stopRun}
              onContextMenu={openLaunchContextMenu}
              title="Stop all"
            >
              <Play className="h-4 w-4" />
              Stop
            </Button>
          ) : (
            <div
              className="flex h-11 overflow-hidden rounded-xl border border-black/10 bg-white text-black shadow-sm"
              title={selectedLaunchTitle}
            >
              <button
                type="button"
                className="flex h-full select-none items-center gap-2 px-5 text-[14px] font-[620] tracking-[-0.014em] transition-colors hover:bg-black/[0.04] disabled:cursor-not-allowed disabled:opacity-70"
                disabled={launchBusy}
                onContextMenu={openLaunchContextMenu}
                onClick={() =>
                  startSelectedRun({
                    runMode: selectedEvent ? selectedEventRunMode : runMode,
                    version: selectedVersion,
                  })
                }
              >
                {launchBusy ? (
                  <LoaderCircle className="h-4 w-4 animate-spin" />
                ) : (
                  <Play className="h-4 w-4" />
                )}
                {selectedLaunchLabel}
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
                className="z-50 min-w-48 rounded-[18px] border border-white/10 bg-[var(--theme-surface)] p-1 shadow-2xl shadow-black/45"
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
                              <Check className="h-4 w-4 text-[var(--theme-accent)]" />
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

          {activeEvents.length > 0 && (
            <div className="w-fit">
              <DropdownMenu.Root modal={false}>
                <DropdownMenu.Trigger asChild>
                  <button
                    type="button"
                    className={cn(
                      "flex h-11 items-center gap-2 rounded-xl border border-panel-outline bg-[var(--theme-surface)] pl-3 pr-2.5 text-[14px] font-medium tracking-[-0.012em] text-white outline-none transition-colors duration-150 hover:bg-white/[0.07] focus:ring-2 focus:ring-panel-outline data-[state=open]:bg-white/[0.08]",
                      selectedEvent ? "border-[var(--theme-accent)]/60" : ""
                    )}
                    aria-label="Select event"
                  >
                    <span className="max-w-[180px] truncate">
                      {selectedEvent?.name ?? "Events"}
                    </span>
                    <ChevronDown className="h-4 w-4 text-white/45" />
                  </button>
                </DropdownMenu.Trigger>
                <DropdownMenu.Portal>
                  <ScrollableDropdownContent
                    sideOffset={8}
                    align="start"
                    className="z-50 min-w-64 rounded-[18px] border border-panel-outline bg-[var(--theme-surface)] p-1 shadow-2xl shadow-black/45"
                    scrollAreaClassName="max-h-72"
                  >
                    {selectedEvent && (
                      <>
                        <DropdownMenu.Item
                          onSelect={() => {
                            const version = Number(selectedVersion);
                            setSelectedEventId("");
                            saveSelectedEventId("");
                            if (Number.isFinite(version)) {
                              invoke("clear_selected_event", { version })
                                .then(() => invoke("get_disabled_mods"))
                                .then((dm) => setDisabledMods(Array.isArray(dm) ? dm : []))
                                .catch((e) => console.error(e));
                            }
                          }}
                          className="flex cursor-pointer select-none items-center gap-2 rounded-xl px-3 py-2 text-[14px] font-medium tracking-[-0.012em] text-white/70 outline-none transition focus:bg-white/10"
                        >
                          <span className="inline-flex w-5 items-center justify-center" />
                          <span>Clear event</span>
                        </DropdownMenu.Item>
                        <DropdownMenu.Separator className="mx-2 my-1 h-px bg-white/20" />
                      </>
                    )}
                    {activeEvents.map((event) => {
                      const active =
                        selectedEvent &&
                        String(selectedEvent.id).toLowerCase() ===
                          String(event.id).toLowerCase();
                      const versionText =
                        Array.isArray(event.versions) && event.versions.length > 0
                          ? event.versions.map((v) => `v${v}`).join(", ")
                          : "all versions";
                      return (
                        <DropdownMenu.Item
                          key={event.id}
                          onSelect={() => {
                            handleEventSelect(event).catch(console.error);
                          }}
                          className={cn(
                            "flex cursor-pointer select-none items-center gap-2 rounded-xl px-3 py-2 text-[14px] font-medium tracking-[-0.012em] text-white/85 outline-none transition focus:bg-white/10",
                            active ? "bg-white/10" : ""
                          )}
                        >
                          <span className="inline-flex w-5 items-center justify-center">
                            {active ? (
                              <Check className="h-4 w-4 text-[var(--theme-accent)]" />
                            ) : null}
                          </span>
                          <span className="min-w-0 flex-1">
                            <span className="block truncate">{event.name}</span>
                            <span className="block truncate text-xs text-white/40">
                              {versionText}
                            </span>
                          </span>
                        </DropdownMenu.Item>
                      );
                    })}
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
                  className="flex h-11 items-center gap-1 rounded-xl border border-panel-outline bg-[var(--theme-surface)] pl-3 pr-2.5 text-[14px] font-medium tracking-[-0.012em] text-white outline-none transition-colors duration-150 hover:bg-white/[0.07] focus:ring-2 focus:ring-panel-outline data-[state=open]:bg-white/[0.08]"
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
                      <CheckCircle2 className="h-4 w-4 text-[var(--theme-accent)]" />
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
                  className="z-50 min-w-[170px] rounded-[18px] border border-panel-outline bg-[var(--theme-surface)] p-1 shadow-2xl shadow-black/45"
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
                            await prepareRunMode(selectedEvent ? selectedEventRunMode : runMode, nextV, {
                              prevRunMode: prevRun,
                              prevVersion: prevVer,
                              eventId: selectedEvent?.id,
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
                            <Check className="h-4 w-4 shrink-0 text-[var(--theme-accent)]" />
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
              className="h-11 bg-[var(--theme-surface)] hover:bg-white/[0.07]"
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
              className="h-11 bg-[var(--theme-surface)] pl-10"
            />
          </div>

          {hasSelectedVersionUpdates && <Button
            variant="secondary"
            className="h-11 bg-[var(--theme-surface)] hover:bg-white/[0.07]"
            onClick={() => {
              const sameContext = checkUpdateMatchesSelectedRun;
              const alreadyChecked =
                sameContext && checkUpdateTask.status === "done";
              const isChecking =
                sameContext && checkUpdateTask.status === "working";

              // Don't re-check if we already have results for this version.
              // If it's currently checking, just open the modal to show progress.
              if (!alreadyChecked && !isChecking) {
                checkModUpdates(selectedVersion, {
                  runMode: selectedEvent ? selectedEventRunMode : runMode,
                });
              }
              setCheckUpdatePrompt({ open: true, mods: filteredMods });
            }}
            title="Check for updates"
          >
            <span className="relative inline-flex">
              <Download className="h-4 w-4" />
              {checkUpdateTask.status === "done" &&
              checkUpdateMatchesSelectedRun &&
              (selectedRunUpdatableMods.length > 0 || hasNativeOverlayUpdate) ? (
                <span className="absolute -right-2 -top-2 rounded-full bg-amber-400 px-1.5 py-0.5 text-[10px] font-bold text-black">
                  {selectedRunUpdatableMods.length + (hasNativeOverlayUpdate ? 1 : 0)}
                </span>
              ) : null}
            </span>
          </Button>}
          {loginState?.username != null && <div className="ml-2 flex items-center gap-2">
            <Button
              variant="ghost"
              size="sm"
              className="h-11 w-11 border border-panel-outline bg-[var(--theme-surface)] p-0 text-white/60 hover:bg-white/[0.07] hover:text-white"
              onClick={() => onLogout?.()}
              title="Logout"
              aria-label="Logout"
            >
              <LogOut className="h-4 w-4" />
            </Button>
          </div>}
        </div>

        {nativeOverlayUpdateError && (
          <div className="flex items-center justify-between gap-3 rounded-xl border border-red-400/30 bg-red-500/10 px-3 py-2 text-sm text-red-100">
            <span>HQ Overlay update failed: {nativeOverlayUpdateError}</span>
            <Button
              variant="outline"
              size="sm"
              className="h-8 shrink-0"
              onClick={() => setNativeOverlayUpdateError("")}
            >
              <X className="h-3.5 w-3.5" />
              Dismiss
            </Button>
          </div>
        )}

        {selectedEvent && (
          <div className="flex min-h-[76px] items-center gap-3 rounded-2xl border border-panel-outline bg-[var(--theme-surface)] p-3">
            {selectedEvent.image ? (
              <div className="h-14 w-24 shrink-0 overflow-hidden rounded-xl border border-white/10 bg-black/30">
                <img
                  src={selectedEvent.image}
                  alt=""
                  className="h-full w-full object-cover"
                  draggable={false}
                />
              </div>
            ) : null}
            <div className="min-w-0 flex-1">
              <div className="flex min-w-0 items-center gap-2">
                <div className="min-w-0 truncate text-sm font-semibold text-white">
                  {selectedEvent.name}
                </div>
                {selectedEventTimeRemaining ? (
                  <div className="inline-flex shrink-0 items-center gap-1 rounded-full border border-white/10 bg-black/20 px-2 py-0.5 text-[11px] font-medium text-white/65">
                    <Timer className="h-3 w-3" />
                    {selectedEventTimeRemaining}
                  </div>
                ) : null}
              </div>
              <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-white/45">
                <span>{selectedRunOption?.label ?? "HQ Run"}</span>
                <span>/</span>
                <span>
                  {Array.isArray(selectedEvent.versions) &&
                  selectedEvent.versions.length > 0
                    ? selectedEvent.versions.map((v) => `v${v}`).join(", ")
                    : "all versions"}
                </span>
                {selectedEventMods.length > 0 ? (
                  <>
                    <span>/</span>
                    <span>{selectedEventMods.length} event mods</span>
                  </>
                ) : null}
              </div>
              <div className="hidden">
                <span>{selectedRunOption?.label ?? "HQ Run"}</span>
                <span>•</span>
                <span>
                  {Array.isArray(selectedEvent.versions) &&
                  selectedEvent.versions.length > 0
                    ? selectedEvent.versions.map((v) => `v${v}`).join(", ")
                    : "all versions"}
                </span>
                {selectedEventMods.length > 0 ? (
                  <>
                    <span>•</span>
                    <span>{selectedEventMods.length} event mods</span>
                  </>
                ) : null}
              </div>
            </div>
            <div className="flex shrink-0 items-center gap-2">
              {selectedEvent.links?.discord ? (
                <Button
                  variant="secondary"
                  className="h-9 w-9 bg-black/20 p-0 hover:bg-white/[0.07]"
                  onClick={() =>
                    invoke("open_external_url", {
                      url: selectedEvent.links.discord,
                    }).catch(console.error)
                  }
                  title="Discord"
                  aria-label="Open Discord"
                >
                  <MessageCircle className="h-4 w-4" />
                </Button>
              ) : null}
              {selectedEvent.links?.website ? (
                <Button
                  variant="secondary"
                  className="h-9 w-9 bg-black/20 p-0 hover:bg-white/[0.07]"
                  onClick={() =>
                    invoke("open_external_url", {
                      url: selectedEvent.links.website,
                    }).catch(console.error)
                  }
                  title="Website"
                  aria-label="Open website"
                >
                  <Globe2 className="h-4 w-4" />
                </Button>
              ) : null}
            </div>
          </div>
        )}

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
              "min-h-0 rounded-2xl border border-panel-outline bg-[var(--theme-surface)] p-3",
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
                  const modSwitchMods = Array.isArray(m?._modSwitchPair)
                    ? m._modSwitchPair
                    : null;
                  const modSwitchIndex = modSwitchMods?.findIndex(
                    (entry) => modKeyLower(entry) === keyLower
                  );
                  const modSwitchTarget =
                    modSwitchMods && modSwitchMods.length > 1
                      ? modSwitchMods[
                          ((modSwitchIndex >= 0 ? modSwitchIndex : 0) + 1) %
                            modSwitchMods.length
                        ]
                      : null;
                  const coverSrc = presetSummary ? m.iconSrc : installedModIconUrls[keyLower];
                  const baseDescription = presetSummary
                    ? m.description
                    : installedModDescriptions[keyLower] || "Click to edit config";
                  const description = baseDescription;
                  const smhqEnableLocked =
                    isSmhqRunMode(runMode) && smhqForcedModKeys.has(keyLower);
                  const eclipsedEnableLocked =
                    isEclipsedRunMode(runMode) && eclipsedForcedModKeys.has(keyLower);
                  const eventEnableLocked = eventForcedModKeys.has(keyLower);
                  const modSwitchLocked = modSwitchMods?.some((entry) => {
                    const entryKey = modKeyLower(entry);
                    return (
                      (isPracticeRunMode(runMode) && practiceLockedModKeys.has(entryKey)) ||
                      (isSmhqRunMode(runMode) && smhqForcedModKeys.has(entryKey)) ||
                      (isEclipsedRunMode(runMode) && eclipsedForcedModKeys.has(entryKey)) ||
                      eventForcedModKeys.has(entryKey)
                    );
                  });
                  const enabled =
                    smhqEnableLocked ||
                    eclipsedEnableLocked ||
                    eventEnableLocked ||
                    (!disabledSet.has(keyLower) && !practiceLockedModKeys.has(keyLower));
                  const installedVer = installedModVersions[keyLower];
                  const busy = modSwitchMods
                    ? modSwitchMods.some((entry) =>
                        modToggleBusyKeys.has(modKeyLower(entry))
                      )
                    : modToggleBusyKeys.has(keyLower);
                  const isPracticeMod =
                    isPracticeRunMode(runMode) && practiceModKeys.has(keyLower);
                  const practiceEnableLocked =
                    isPracticeRunMode(runMode) && practiceLockedModKeys.has(keyLower);
                  const needsGoogleOauth = requiresGoogleOauthForMod(m);
                  return (
                    <div
                      key={listEntryKey(m)}
                      className={cn(
                        "group flex w-full items-center gap-3 rounded-2xl border px-3 py-3 text-left transition",
                        selected
                          ? "border-panel-outline bg-white/[0.08]"
                          : "border-panel-outline bg-black/20 hover:bg-white/[0.07]",
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
                      <ModCover
                        src={coverSrc}
                        initials={initials}
                        onMissing={() => forgetInstalledModIcon(keyLower)}
                      />
                      <div className="min-w-0 flex-1">
                        <div className="flex items-baseline gap-2">
                          <div className="truncate text-base font-semibold">
                            {m.name}
                          </div>
                          <div className="truncate text-sm text-white/40">
                            {m.dev}
                          </div>
                          {modSwitchTarget ? (
                            <button
                              type="button"
                              className="inline-flex min-w-0 max-w-[170px] shrink items-center gap-1.5 rounded-full border border-white/10 bg-white/[0.05] px-2 py-0.5 text-[11px] font-medium text-white/60 transition hover:border-white/20 hover:bg-white/[0.09] hover:text-white/85 disabled:cursor-not-allowed disabled:opacity-40"
                              disabled={
                                busy ||
                                modSwitchLocked ||
                                practiceEnableLocked ||
                                smhqEnableLocked ||
                                eclipsedEnableLocked ||
                                eventEnableLocked
                              }
                              title={`Switch to ${modSwitchTarget.dev}-${modSwitchTarget.name}`}
                              aria-label={`Switch to ${modSwitchTarget.dev}-${modSwitchTarget.name}`}
                              onClick={(event) => {
                                event.stopPropagation();
                                switchModVariant(m, modSwitchTarget).catch(console.error);
                              }}
                            >
                              {busy ? (
                                <LoaderCircle className="h-3 w-3 shrink-0 animate-spin" />
                              ) : (
                                <ArrowLeftRight className="h-3 w-3 shrink-0" />
                              )}
                              <span className="truncate">{modSwitchTarget.name}</span>
                            </button>
                          ) : null}
                          {/* {presetSummary ? (
                            <div className="rounded-full border border-sky-400/30 bg-sky-400/10 px-2 py-0.5 text-[11px] font-medium text-sky-100">
                              Locked
                            </div>
                          ) : null} */}
                          {isPracticeMod ? (
                            <div className="rounded-full border border-[color-mix(in_srgb,var(--theme-accent)_35%,transparent)] bg-[var(--theme-accent-muted)] px-2 py-0.5 text-[11px] font-medium text-[var(--theme-accent)]">
                              Practice
                            </div>
                          ) : null}
                          {smhqEnableLocked ? (
                            <div className="rounded-full border border-sky-400/30 bg-sky-400/10 px-2 py-0.5 text-[11px] font-medium text-sky-200">
                              SMHQ
                            </div>
                          ) : null}
                          {eclipsedEnableLocked ? (
                            <div className="rounded-full border border-violet-400/30 bg-violet-400/10 px-2 py-0.5 text-[11px] font-medium text-violet-200">
                              Eclipsed
                            </div>
                          ) : null}
                          {eventEnableLocked ? (
                            <div className="rounded-full border border-emerald-400/30 bg-emerald-400/10 px-2 py-0.5 text-[11px] font-medium text-emerald-200">
                              Event
                            </div>
                          ) : null}
                          {needsGoogleOauth ? (
                            <div className="rounded-full border border-amber-400/30 bg-amber-400/10 px-2 py-0.5 text-[11px] font-medium text-amber-200">
                              Google
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
                          <div className="rounded-full border border-white/10 bg-black/20 px-2.5 py-1 text-[11px] font-medium text-white/65">
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
                              disabled={
                                busy ||
                                modSwitchLocked ||
                                practiceEnableLocked ||
                                smhqEnableLocked ||
                                eclipsedEnableLocked ||
                                eventEnableLocked
                              }
                              onCheckedChange={(v) =>
                                toggleModEnabledForMod(m, !!v, {
                                  propagateChain: !modSwitchTarget,
                                })
                              }
                            />
                          </div>
                        )}
                      </div>
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
                  data-resize-handle="true"
                  className="group absolute inset-y-0 left-1/2 w-6 -translate-x-1/2 cursor-ew-resize"
                  onPointerDown={startPanelResize}
                >
                  <span
                    className={cn(
                      "pointer-events-none absolute inset-y-0 left-1/2 w-px -translate-x-1/2 bg-[#2C313A] transition-colors",
                      isResizingPanels
                        ? "bg-[var(--theme-accent)]"
                        : "group-hover:bg-[#434B58]"
                    )}
                  />
                  <span
                    className={cn(
                      "pointer-events-none absolute left-1/2 top-1/2 h-14 w-1.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-[#3A404A] transition-colors",
                      isResizingPanels
                        ? "bg-[var(--theme-accent)]"
                        : "group-hover:bg-[#596273]"
                    )}
                  />
                </button>
              </div>
            ) : null}
            <div
              className={cn(
                "min-h-0 rounded-2xl border border-panel-outline bg-[var(--theme-surface)] p-4",
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
                      <div className="min-h-0 flex-1 overflow-auto rounded-2xl border border-panel-outline bg-[var(--theme-surface)] p-3">
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
                                  className="flex items-center gap-3 rounded-2xl border border-panel-outline bg-black/20 px-3 py-3"
                                  onContextMenu={(e) => openModContextMenu(e, mod)}
                                >
                                  <ModCover
                                    src={iconSrc}
                                    initials={`${mod.dev?.[0] ?? "M"}${
                                      mod.name?.[0] ?? "M"
                                    }`.toUpperCase()}
                                    onMissing={() => forgetInstalledModIcon(keyLower)}
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
                ) : selectedLcStatsTracker ? (
                  renderLcStatsSettingsPanel()
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
                    <div className="min-h-0 flex-1 overflow-auto rounded-2xl border border-panel-outline bg-[var(--theme-surface)] p-3">
                    
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
                              const lockedEntry = isLockedCfgEntry(
                                activeConfigPath,
                                s.name,
                                e.name
                              );
                              return (
                                <div
                                  key={id}
                                  className="rounded-2xl border border-panel-outline bg-black/15 p-3"
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
                                      <label
                                        className={cn(
                                          "flex cursor-pointer items-center gap-2 text-sm",
                                          lockedEntry && "cursor-not-allowed opacity-60"
                                        )}
                                      >
                                        <Checkbox
                                          checked={!!v.data}
                                          disabled={lockedEntry}
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
                                            disabled={lockedEntry}
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
                                          disabled={lockedEntry}
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
                                            disabled={lockedEntry}
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
                                            disabled={lockedEntry}
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
                                          disabled={lockedEntry}
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
                                        disabled={lockedEntry}
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
                                                className={cn(
                                                  "flex cursor-pointer items-center gap-2 text-sm text-white/80",
                                                  lockedEntry && "cursor-not-allowed opacity-60"
                                                )}
                                              >
                                                <Checkbox
                                                  checked={checked}
                                                  disabled={lockedEntry}
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
                                        disabled={lockedEntry}
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
          className="fixed z-[70] min-w-[220px] overflow-hidden rounded-2xl border border-panel-outline bg-[var(--theme-surface)] p-1.5 shadow-2xl shadow-black/40"
          style={{
            left: modContextMenu.x,
            top: modContextMenu.y,
          }}
        >
          <button
            className="flex w-full items-center gap-2 rounded-xl px-3 py-2 text-left text-sm text-white/85 transition hover:bg-white/[0.07]"
            onClick={() => openSelectedModFolder(modContextMenu.mod)}
          >
            <FolderOpen className="h-4 w-4 text-white/50" />
            Open Mod Folder
          </button>
          <button
            className="flex w-full items-center gap-2 rounded-xl px-3 py-2 text-left text-sm text-white/85 transition hover:bg-white/[0.07] disabled:cursor-not-allowed disabled:opacity-35 disabled:hover:bg-transparent"
            disabled={!modContextMenu.configPath}
            onClick={() => openSelectedConfigFile(modContextMenu.configPath)}
          >
            <FileCog className="h-4 w-4 text-white/50" />
            Open Config File
          </button>
          <button
            className="flex w-full items-center gap-2 rounded-xl px-3 py-2 text-left text-sm text-white/85 transition hover:bg-white/[0.07]"
            onClick={() => openSelectedModThunderstore(modContextMenu.mod)}
          >
            <Globe2 className="h-4 w-4 text-white/50" />
            Open Thunderstore
          </button>
        </div>
      )}

      {versionContextMenu.open && Number.isFinite(versionContextMenu.version) && (
        <div
          ref={versionContextMenuRef}
          className="fixed z-[70] min-w-[220px] overflow-hidden rounded-2xl border border-panel-outline bg-[var(--theme-surface)] p-1.5 shadow-2xl shadow-black/40"
          style={{
            left: versionContextMenu.x,
            top: versionContextMenu.y,
          }}
        >
          <button
            className="flex w-full items-center rounded-xl px-3 py-2 text-left text-sm text-white/85 transition hover:bg-white/[0.07] disabled:pointer-events-none disabled:opacity-40"
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

      {launchContextMenu.open && (
        <div
          ref={launchContextMenuRef}
          className="fixed z-[70] min-w-[220px] overflow-hidden rounded-2xl border border-panel-outline bg-[var(--theme-surface)] p-1.5 shadow-2xl shadow-black/40"
          style={{
            left: launchContextMenu.x,
            top: launchContextMenu.y,
          }}
        >
          <button
            className="flex w-full items-center rounded-xl px-3 py-2 text-left text-sm text-white/85 transition hover:bg-white/[0.07] disabled:pointer-events-none disabled:text-white/30 disabled:opacity-60"
            disabled={
              !gameStatus.running ||
              launchBusy ||
              !isInstalled(Number(selectedVersion))
            }
            onClick={() => {
              closeLaunchContextMenu();
              startSelectedRun({
                runMode: selectedEvent ? selectedEventRunMode : runMode,
                version: selectedVersion,
                allowMultiple: true,
              });
            }}
          >
            Launch Another Game
          </button>
          {gameStatus.running ? (
            <button
              className="flex w-full items-center rounded-xl px-3 py-2 text-left text-sm text-white/85 transition hover:bg-white/[0.07]"
              onClick={() => {
                closeLaunchContextMenu();
                openRunningGamesDialog();
              }}
            >
              Manage Running Games
            </button>
          ) : null}
        </div>
      )}

      <Dialog open={runningGamesDialogOpen} onOpenChange={setRunningGamesDialogOpen}>
        <DialogContent className="w-[min(760px,94vw)] p-0">
          <div className="flex items-center justify-between border-b border-white/10 px-5 py-4">
            <div>
              <div className="text-lg font-semibold">Running Games</div>
              <div className="mt-1 text-sm text-white/50">
                Manage game instances launched from this session.
              </div>
            </div>
            <button
              type="button"
              onClick={() => setRunningGamesDialogOpen(false)}
              className="rounded-lg p-2 text-white/60 transition hover:bg-white/[0.07] hover:text-white"
              aria-label="Close running games"
            >
              <X className="h-4 w-4" />
            </button>
          </div>

          <div className="max-h-[min(62vh,560px)] overflow-y-auto px-5 py-4">
            {runningGames.length === 0 ? (
              <div className="rounded-2xl border border-panel-outline bg-black/20 px-4 py-5 text-sm text-white/55">
                No running games.
              </div>
            ) : (
              <div className="space-y-3">
                {runningGames.map((game) => {
                  const options = Array.isArray(game.launch_options)
                    ? game.launch_options
                    : [];
                  const template = String(game.launch_command_template ?? "").trim();
                  return (
                    <div
                      key={game.id}
                      className="rounded-2xl border border-panel-outline bg-white/[0.04] p-4"
                    >
                      <div className="flex items-start justify-between gap-3">
                        <div className="min-w-0">
                          <div className="flex flex-wrap items-center gap-2">
                            <span className="text-sm font-semibold text-white">
                              #{game.order} {game.mode_label || "Game"}
                            </span>
                            <span className="rounded-full bg-white/10 px-2 py-0.5 text-xs text-white/65">
                              v{game.version}
                            </span>
                            {typeof game.pid === "number" ? (
                              <span className="rounded-full bg-white/10 px-2 py-0.5 text-xs text-white/65">
                                PID {game.pid}
                              </span>
                            ) : null}
                          </div>
                          <div className="mt-3 space-y-2 text-xs text-white/55">
                            <div>
                              <span className="text-white/35">Command wrapper</span>
                              <div className="mt-1 rounded-lg bg-black/20 px-2 py-1 font-mono text-white/70">
                                {template || "Default"}
                              </div>
                            </div>
                            <div>
                              <span className="text-white/35">Launch options</span>
                              <div className="mt-1 rounded-lg bg-black/20 px-2 py-1 font-mono text-white/70">
                                {options.length > 0 ? options.join("  ") : "None"}
                              </div>
                            </div>
                          </div>
                        </div>
                        <Button
                          variant="secondary"
                          className="h-9 shrink-0 px-3"
                          disabled={runningGameStopBusyId === game.id}
                          onClick={() => stopRunningGame(game.id)}
                          title="Stop this game"
                        >
                          {runningGameStopBusyId === game.id ? (
                            <LoaderCircle className="h-4 w-4 animate-spin" />
                          ) : (
                            <X className="h-4 w-4" />
                          )}
                          Stop
                        </Button>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        </DialogContent>
      </Dialog>

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

          <div className="relative w-[min(520px,calc(100vw-2rem))] rounded-2xl border border-panel-outline bg-[var(--theme-surface)] p-5 shadow-2xl shadow-black/50">
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
              <TaskProgressPanel
                title={statusText}
                detail={task.detail || (bytesText ? `Downloaded: ${bytesText}` : "")}
                task={task}
                error={task.error}
                percent={task.overall_percent}
                percentText={progressText}
                barError={task.status === "error"}
              />
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

          <div className="relative w-[min(520px,calc(100vw-2rem))] rounded-2xl border border-panel-outline bg-[var(--theme-surface)] p-5 shadow-2xl shadow-black/50">
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
              <TaskProgressPanel
                title={deleteVersionPrompt.detail || "Deleting files..."}
                detail={
                  Number.isFinite(Number(deleteVersionPrompt.total_files)) &&
                  Number(deleteVersionPrompt.total_files) > 0
                    ? `${Number(deleteVersionPrompt.deleted_files ?? 0)} / ${Number(
                        deleteVersionPrompt.total_files ?? 0
                      )} items removed`
                    : "Scanning files..."
                }
                task={null}
                percent={deleteVersionPrompt.overall_percent}
                percentText={`${Math.round(Number(deleteVersionPrompt.overall_percent ?? 0))}%`}
              />
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

          <div className="relative w-[min(520px,calc(100vw-2rem))] rounded-2xl border border-panel-outline bg-[var(--theme-surface)] p-5 shadow-2xl shadow-black/50">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <div className="text-lg font-semibold">
                  {checkUpdateTask.status === "done"
                    ? (() => {
                        const overlayAvailable =
                          nativeOverlayUpdate?.supported !== false &&
                          nativeOverlayUpdate?.available === true;
                        const modCount = selectedRunUpdatableMods.length;
                        if (modCount === 0 && overlayAvailable) {
                          return "HQ Overlay update available";
                        }
                        if (modCount === 0) {
                          return "Everything is up to date";
                        }
                        return overlayAvailable
                          ? `${modCount} mods + HQ Overlay can be updated`
                          : `${modCount} mods can be updated`;
                      })()
                    : "Checking for updates..."}
                </div>
              </div>
            </div>

            {(checkUpdateTask.status === "working" ||
              checkUpdateTask.status === "error") && (
              <div className="mt-4 rounded-2xl">
                <ProgressBar
                  percent={checkUpdateTask.overall_percent}
                  error={checkUpdateTask.status === "error"}
                  trackClassName="bg-gray-500"
                />

                <div className="mt-2 flex items-center justify-between gap-3 text-sm text-white/50">
                  {checkUpdateTask.detail}
                </div>
              </div>
            )}
            {checkUpdateTask.status === "done" && (
              <div className="mt-4 space-y-1 text-sm text-white/50">
                {selectedRunUpdatableMods.map((mod, index) => (
                  <div key={index}>{mod}</div>
                ))}
                {nativeOverlayUpdate?.supported !== false &&
                  nativeOverlayUpdate?.available === true && (
                    <div key="hq-overlay">
                      HQ Overlay: {nativeOverlayUpdate.current_version ?? "not installed"}{" "}
                      &rarr; {nativeOverlayUpdate.latest_version ?? "latest"}
                    </div>
                  )}
                {selectedRunUpdatableMods.length === 0 &&
                  !(
                    nativeOverlayUpdate?.supported !== false &&
                    nativeOverlayUpdate?.available === true
                  ) && <div>Everything is up to date</div>}
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
                      disabled={
                        selectedRunUpdatableMods.length === 0 &&
                        !(
                          nativeOverlayUpdate?.supported !== false &&
                          nativeOverlayUpdate?.available === true
                        )
                      }
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

          <div className="relative w-[min(520px,calc(100vw-2rem))] rounded-2xl border border-panel-outline bg-[var(--theme-surface)] p-5 shadow-2xl shadow-black/50">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <div className="text-lg font-semibold">
                  {updateIsStorageMove && updateIsDone
                    ? "Game storage moved"
                    : updateIsStorageMove && updateIsError
                    ? "Move failed"
                    : updateIsStorageMove
                    ? "Moving game storage..."
                    : manifestUpdateInfo && !updateIsWorking && !updateIsDone && !updateIsError
                    ? "Update available"
                    : updateIsDone
                    ? "Update complete"
                    : updateIsError
                    ? "Update failed"
                    : "Updating..."}
                </div>
                <div className="mt-1 text-sm text-white/55">
                  {updateIsStorageMove
                    ? "Installed game versions are being moved to the selected storage folder."
                    : manifestUpdateInfo && !updateIsWorking && !updateIsDone && !updateIsError
                    ? `v${manifestUpdateInfo.version ?? selectedVersion} version's allowed manifest has changed.`
                    : "Based on the remote manifest, installed game and mod files are being synced to the desired state."}
                </div>
              </div>
            </div>

            {manifestUpdateInfo && !updateIsWorking && !updateIsDone && !updateIsError && (
              <div className="mt-4 rounded-2xl border border-panel-outline bg-black/20 p-3 text-sm text-white/70">
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
              <TaskProgressPanel
                title={statusText}
                detail={task.detail || ""}
                task={task}
                error={task.error}
                percent={task.overall_percent}
                percentText={progressText}
                barError={updateIsError}
              />
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

          <div className="relative w-[min(520px,calc(100vw-2rem))] rounded-2xl border border-panel-outline bg-[var(--theme-surface)] p-5 shadow-2xl shadow-black/50">
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

            <TaskProgressPanel
              title={practiceTask?.step_name ?? "Practice Mods"}
              detail={practiceTask?.detail ?? ""}
              task={practiceTask}
              error={practiceTask?.error}
              percent={practiceTask?.overall_percent}
              percentText={
                Number.isFinite(Number(practiceTask?.overall_percent))
                  ? `${Math.round(Number(practiceTask?.overall_percent))}%`
                  : ""
              }
              barError={practiceTask?.status === "error"}
            />

            <PrepareCancelButton
              task={practiceTask}
              setTask={setPracticeTask}
              cancelBusy={practiceCancelBusy}
              setCancelBusy={setPracticeCancelBusy}
              onClose={() => setPracticePrompt({ open: false })}
              onCancelStart={() => {
                explicitCancelKeyRef.current = latestPrepareKeyRef.current;
              }}
              fallbackVersion={selectedVersion}
            />
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

          <div className="relative w-[min(520px,calc(100vw-2rem))] rounded-2xl border border-panel-outline bg-[var(--theme-surface)] p-5 shadow-2xl shadow-black/50">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <div className="text-lg font-semibold">
                  {presetTask?.status === "error"
                    ? presetTask?.step_name === "Event Mods"
                      ? "Event setup failed"
                      : "Preset setup failed"
                    : presetTask?.step_name === "Event Mods"
                      ? "Installing event mods..."
                      : "Installing preset mods..."}
                </div>
                <div className="mt-1 text-sm text-white/55">
                  {presetTask?.step_name === "Event Mods"
                    ? "Event plugins are being installed for this run."
                    : "Preset-tagged plugins are being installed for this run."}
                </div>
              </div>
            </div>

            <TaskProgressPanel
              title={presetTask?.step_name ?? "Preset Mods"}
              detail={presetTask?.detail ?? ""}
              task={presetTask}
              error={presetTask?.error}
              percent={presetTask?.overall_percent}
              percentText={
                Number.isFinite(Number(presetTask?.overall_percent))
                  ? `${Math.round(Number(presetTask?.overall_percent))}%`
                  : ""
              }
              barError={presetTask?.status === "error"}
            />

            <PrepareCancelButton
              task={presetTask}
              setTask={setPresetTask}
              cancelBusy={presetCancelBusy}
              setCancelBusy={setPresetCancelBusy}
              onClose={() => setPresetPrompt({ open: false })}
              onCancelStart={() => {
                explicitCancelKeyRef.current = latestPrepareKeyRef.current;
              }}
              fallbackVersion={selectedVersion}
            />
          </div>
        </div>
      )}

      <Dialog
        open={launchOptionsDialogOpen}
        onOpenChange={setLaunchOptionsDialogOpen}
      >
        <DialogContent className="max-h-[calc(100vh-4.5rem)] w-[min(720px,94vw)] overflow-hidden p-0">
          <div className="flex max-h-[calc(100vh-4.5rem)] flex-col rounded-3xl border border-panel-outline bg-[var(--theme-surface)] text-white">
            <div className="flex items-start justify-between gap-4">
              <div className="p-6 pb-0">
                <div className="text-lg font-semibold tracking-[-0.02em]">
                  Launch
                </div>
                <div className="mt-1 text-sm text-white/55">
                  Add custom launch options. Use one row per entry. <span className="font-mono">KEY=VALUE</span> becomes an environment variable; anything else is passed as one extra launch token. You can also wrap the full game command with a template such as <span className="font-mono">gamescope -f -r 144 -- %command%</span>.
                </div>
              </div>
              <button
                type="button"
                className="m-6 mb-0 rounded-xl border border-panel-outline bg-black/20 p-2 text-white/70 transition hover:bg-white/[0.07] hover:text-white"
                onClick={() => {
                  setLaunchOptionsDialogOpen(false);
                }}
                aria-label="Close launch options"
              >
                <X className="h-4 w-4" />
              </button>
            </div>

            <div className="mt-6 flex-1 space-y-5 overflow-y-auto px-6 pb-6">
              <div className="flex items-center justify-between gap-4 rounded-2xl border border-panel-outline bg-black/20 px-4 py-3">
                <div>
                  <div className="text-sm font-medium text-white">
                    Enable custom launch options
                  </div>
                  <div className="mt-1 text-xs text-white/50">
                    Saved locally and applied to every run mode until you turn them off.
                  </div>
                </div>
                <Switch
                  checked={launchOptionsEnabled}
                  onCheckedChange={setLaunchOptionsEnabled}
                />
              </div>

              <div className="space-y-3">
                <div className="space-y-2 rounded-2xl border border-panel-outline bg-white/[0.04] p-4">
                  <div>
                    <div className="text-sm font-medium text-white">
                      Command template
                    </div>
                    <div className="mt-1 text-xs text-white/50">
                      Use <span className="font-mono">%command%</span> where the launcher should insert the default game command. This is the right place for wrappers that should run before the game command, like <span className="font-mono">gamescope</span>. If omitted, the launcher appends the game command to the end.
                    </div>
                  </div>
                  <Input
                    value={launchCommandTemplate}
                    onChange={(event) => {
                      setLaunchCommandTemplate(event.target.value);
                    }}
                    placeholder="gamescope -f -r 144 -- %command%"
                    className="font-mono"
                  />
                </div>

                {launchOptionsEntries.length > 0 ? (
                  launchOptionsEntries.map((entry, index) => (
                    <div
                      key={index}
                      className="flex items-center gap-2"
                    >
                      <button
                        type="button"
                        className="inline-flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border border-panel-outline bg-black/20 text-white/60 transition hover:bg-white/[0.07] hover:text-white"
                        onClick={() => {
                          removeLaunchOptionEntry(index);
                        }}
                        aria-label={`Remove launch option ${index + 1}`}
                      >
                        <X className="h-4 w-4" />
                      </button>
                      <Input
                        value={entry}
                        onChange={(event) => {
                          updateLaunchOptionEntry(index, event.target.value);
                        }}
                        placeholder="DXVK_ASYNC=1"
                        className="font-mono"
                      />
                    </div>
                  ))
                ) : (
                  <div className="rounded-2xl border border-dashed border-white/15 bg-white/[0.03] px-4 py-5 text-sm text-white/45">
                    No custom launch options yet.
                  </div>
                )}

                <div>
                  <Input
                    value={newLaunchOptionEntry}
                    onChange={(event) => {
                      setNewLaunchOptionEntry(event.target.value);
                    }}
                    onKeyDown={(event) => {
                      if (event.key !== "Enter") return;
                      event.preventDefault();
                      addLaunchOptionEntry();
                    }}
                    placeholder="Enter env var or extra launch token..."
                    className="font-mono"
                  />
                </div>
              </div>
            </div>

            <div className="flex items-center justify-end gap-2 border-t border-panel-outline px-6 py-4">
              <Button
                variant="secondary"
                className="h-10 min-w-[96px]"
                onClick={() => {
                  setLaunchOptionsDialogOpen(false);
                }}
              >
                Close
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>

      <Dialog
        open={googleOauthDialogOpen}
        onOpenChange={(open) => {
          if (!open) {
            cancelGoogleOauthLogin();
          } else {
            setGoogleOauthDialogOpen(true);
          }
        }}
      >
        <DialogContent className="w-[min(640px,92vw)] p-0">
          <div className="rounded-3xl border border-panel-outline bg-[var(--theme-surface)] p-6 text-white">
            <div className="flex items-start justify-between gap-4">
              <div>
                <div className="text-lg font-semibold tracking-[-0.02em]">
                  Google Login
                </div>
                <div className="mt-1 text-sm text-white/55">
                  LCStatsTracker needs access to the selected Google Sheets file.
                </div>
              </div>
              <button
                type="button"
                className="rounded-xl border border-panel-outline bg-black/20 p-2 text-white/70 transition hover:bg-white/[0.07] hover:text-white"
                onClick={cancelGoogleOauthLogin}
                aria-label="Close Google login"
              >
                <X className="h-4 w-4" />
              </button>
            </div>

            {renderCustomGoogleOauthFields({ compact: true })}

            <div className="mt-6 rounded-2xl border border-panel-outline bg-white/[0.04] px-4 py-3">
              <div className="text-sm font-medium text-white">
                {googleOauthStatus.authenticated ? "Connected" : "Not connected"}
              </div>
              <div className="mt-1 text-xs text-white/50">
                {pendingGoogleOauthToggle?.mod
                  ? `${pendingGoogleOauthToggle.mod.dev}-${pendingGoogleOauthToggle.mod.name}`
                  : "LCStatsTracker-MikuOreo"}
              </div>
            </div>

            {googleOauthError ? (
              <div className="mt-4 rounded-2xl border border-red-400/30 bg-red-400/10 px-4 py-3 text-sm text-red-200">
                {googleOauthError}
              </div>
            ) : null}

            <div className="mt-6 flex items-center justify-between gap-2">
              <Button
                variant="secondary"
                className="h-10"
                disabled={googleOauthBusy || !googleOauthStatus.authenticated}
                onClick={() => {
                  logoutGoogleOauth().catch(console.error);
                }}
              >
                <LogOut className="h-4 w-4" />
                Logout
              </Button>
              <div className="flex items-center gap-2">
                {pendingGoogleOauthToggle?.mod ? (
                  <Button
                    variant="secondary"
                    className="h-10 min-w-[160px]"
                    disabled={googleOauthBusy}
                    onClick={() => {
                      enablePendingLcstatsWithoutGoogle().catch(console.error);
                    }}
                  >
                    Enable without Google
                  </Button>
                ) : null}
                <Button
                  variant="secondary"
                  className="h-10 min-w-[96px]"
                  onClick={cancelGoogleOauthLogin}
                >
                  Cancel
                </Button>
                <Button
                  variant="default"
                  className="h-10 min-w-[132px]"
                  disabled={googleOauthBusy}
                  onClick={() => {
                    startGoogleOauthLogin().catch(console.error);
                  }}
                >
                  {googleOauthBusy ? (
                    <>
                      <LoaderCircle className="h-4 w-4 animate-spin" />
                      Waiting...
                    </>
                  ) : (
                    "Login with Google"
                  )}
                </Button>
              </div>
            </div>
          </div>
        </DialogContent>
      </Dialog>

    </div>
  );
}
