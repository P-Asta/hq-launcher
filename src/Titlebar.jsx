import { getCurrentWindow } from '@tauri-apps/api/window';
import { useEffect, useMemo, useState } from 'react';
import { Beaker, Copy, FolderOpen, HardDrive, Minus, Moon, Paintbrush, Play, RefreshCw, RotateCcw, Settings, Square, Sun, X } from 'lucide-react';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import { cn } from './lib/cn';
import { invoke } from '@tauri-apps/api/core';
import { emit, listen } from '@tauri-apps/api/event';
import { isRegistered, register, unregister, unregisterAll } from '@tauri-apps/plugin-global-shortcut';
import { Dialog, DialogContent } from './components/ui/dialog';
import { Button } from './components/ui/button';
import { Switch } from './components/ui/switch';
import {
    DEFAULT_THEME_HUE,
    DEFAULT_THEME_BRIGHTNESS,
    DEFAULT_THEME_MODE,
    loadStoredThemeBrightness,
    loadStoredThemeHue,
    loadStoredThemeMode,
    normalizeThemeBrightness,
    normalizeThemeHue,
    normalizeThemeMode,
    persistAndBroadcastThemeBrightness,
    persistAndBroadcastThemeHue,
    persistAndBroadcastThemeMode,
} from './lib/theme';

const SHOW_THEME_SETTINGS = true;
const SHOW_DEV_SETTINGS = import.meta.env.DEV;


export default function Titlebar({ installedVersions, ...props }) {
    const [isMaximized, setIsMaximized] = useState(false);
    const [configLinkState, setConfigLinkState] = useState(null);
    const [configLinkBusy, setConfigLinkBusy] = useState(false);
    const [linkWarnOpen, setLinkWarnOpen] = useState(false);
    const [settingsOpen, setSettingsOpen] = useState(false);
    const [settingsTab, setSettingsTab] = useState("general");
    const [releaseChannel, setReleaseChannel] = useState(null);
    const [releaseChannelBusy, setReleaseChannelBusy] = useState(false);
    const [releaseChannelError, setReleaseChannelError] = useState("");
    const [gameStorage, setGameStorage] = useState(null);
    const [gameStorageBusy, setGameStorageBusy] = useState(false);
    const [gameStorageError, setGameStorageError] = useState("");
    const [themeHue, setThemeHue] = useState(() => loadStoredThemeHue());
    const [themeBrightness, setThemeBrightness] = useState(() => loadStoredThemeBrightness());
    const [themeMode, setThemeMode] = useState(() => loadStoredThemeMode());
    const [steamOverlayConfig, setSteamOverlayConfig] = useState({
        enabled: false,
        steam_path: "",
    });
    const [gameOverlayConfig, setGameOverlayConfig] = useState({
        general: {
            enabled: true,
            use_stream_overlays_api: false,
            overlay_key: "Insert",
            end_summary_duration_ms: 10000,
        },
    });
    const [steamOverlayResolvedPath, setSteamOverlayResolvedPath] = useState("");
    const [steamOverlayBusy, setSteamOverlayBusy] = useState(false);
    const [obsOverlayBusy, setObsOverlayBusy] = useState(false);
    const [steamOverlayError, setSteamOverlayError] = useState("");
    const [steamOverlaySaved, setSteamOverlaySaved] = useState("");
    const [latestLcstatsPayload, setLatestLcstatsPayload] = useState(null);
    const [latestLcstatsBusy, setLatestLcstatsBusy] = useState(false);
    const [latestLcstatsError, setLatestLcstatsError] = useState("");
    const [latestLcstatsCopied, setLatestLcstatsCopied] = useState(false);
    const [selectedVersion, setSelectedVersion] = useState(null);
    const normalizedThemeHue = useMemo(
        () => normalizeThemeHue(themeHue),
        [themeHue]
    );
    const normalizedThemeBrightness = useMemo(
        () => normalizeThemeBrightness(themeBrightness),
        [themeBrightness]
    );
    const normalizedThemeMode = useMemo(
        () => normalizeThemeMode(themeMode),
        [themeMode]
    );
    
    const appWindow = getCurrentWindow();
    useEffect(() => {
        const handleResize = async () => {
            const maximized = await appWindow.isMaximized();
            setIsMaximized(maximized);
        };

        handleResize();
        const unlisten = appWindow.onResized(() => {
            handleResize();
        });

        return () => {
            unlisten.then(fn => fn());
        };
    }, []);
    const handleMinimize = () => appWindow.minimize();
    const handleMaximize = () => {
        appWindow.toggleMaximize();
        setIsMaximized(!isMaximized);
    };
    const handleClose = async () => {
        try {
            await unregisterAll();
        } catch {}
        appWindow.close();
    };

    async function refreshReleaseChannel() {
        try {
            const channel = await invoke('get_release_channel');
            setReleaseChannel(channel ?? null);
            setReleaseChannelError("");
        } catch (e) {
            console.warn('Failed to read release channel', e);
            setReleaseChannelError(e?.message ?? String(e));
        }
    }

    useEffect(() => {
        refreshReleaseChannel();
        refreshGameStorage();
        refreshOverlaySettings();
    }, []);

    async function refreshOverlaySettings() {
        try {
            const [cfg, overlayCfg] = await Promise.all([
                invoke('get_steam_overlay_config'),
                invoke('get_game_overlay_config'),
            ]);
            setSteamOverlayResolvedPath(String(cfg?.resolved_steam_path ?? ""));
            setSteamOverlayConfig({
                enabled: !!cfg?.enabled,
                steam_path: String(cfg?.steam_path ?? cfg?.resolved_steam_path ?? ""),
            });
            setGameOverlayConfig((prev) => ({
                ...prev,
                ...(overlayCfg ?? {}),
                general: {
                    ...(prev.general ?? {}),
                    ...(overlayCfg?.general ?? {}),
                    enabled: overlayCfg?.general?.enabled !== false,
                },
            }));
            setSteamOverlayError("");
        } catch (e) {
            console.warn('Failed to read overlay settings', e);
            setSteamOverlayError(e?.message ?? String(e));
        }
    }

    async function refreshGameStorage() {
        try {
            const settings = await invoke('get_game_storage_settings');
            setGameStorage(settings ?? null);
            setGameStorageError("");
        } catch (e) {
            console.warn('Failed to read game storage settings', e);
            setGameStorageError(e?.message ?? String(e));
        }
    }

    async function setBetaEnabled(enabled) {
        setReleaseChannelBusy(true);
        setReleaseChannelError("");
        try {
            const channel = await invoke('set_release_channel', {
                channel: enabled ? 'beta' : 'stable',
            });
            setReleaseChannel(channel ?? null);
            await emit('release-channel://changed', channel ?? null);
        } catch (e) {
            setReleaseChannelError(e?.message ?? String(e));
        } finally {
            setReleaseChannelBusy(false);
        }
    }

    async function applyThemeHueValue(nextHue) {
        const applied = await persistAndBroadcastThemeHue(nextHue);
        setThemeHue(applied);
    }

    async function applyThemeBrightnessValue(nextBrightness) {
        const applied = await persistAndBroadcastThemeBrightness(nextBrightness);
        setThemeBrightness(applied);
    }

    async function changeGameStorageDir() {
        if (gameStorageBusy) return;
        setGameStorageBusy(true);
        setGameStorageError("");
        try {
            const picked = await invoke('pick_game_storage_dir', {
                initialPath: gameStorage?.custom_dir ?? gameStorage?.current_dir ?? null,
            });
            if (!picked) return;
            handleSettingsOpenChange(false);
            const settings = await invoke('set_game_storage_dir', { customDir: picked });
            setGameStorage(settings ?? null);
        } catch (e) {
            const message = e?.message ?? String(e);
            setGameStorageError(message);
            window.alert(message);
        } finally {
            setGameStorageBusy(false);
        }
    }

    async function resetGameStorageDir() {
        if (gameStorageBusy) return;
        setGameStorageBusy(true);
        setGameStorageError("");
        try {
            handleSettingsOpenChange(false);
            const settings = await invoke('set_game_storage_dir', { customDir: null });
            setGameStorage(settings ?? null);
        } catch (e) {
            const message = e?.message ?? String(e);
            setGameStorageError(message);
            window.alert(message);
        } finally {
            setGameStorageBusy(false);
        }
    }

    async function refreshLatestLcstatsPayload() {
        if (!SHOW_DEV_SETTINGS) return;
        setLatestLcstatsBusy(true);
        setLatestLcstatsError("");
        try {
            const payload = await invoke('get_lcstats_latest_payload');
            setLatestLcstatsPayload(payload ?? null);
        } catch (e) {
            console.warn('Failed to read latest LCStatsTracker payload', e);
            setLatestLcstatsError(e?.message ?? String(e));
        } finally {
            setLatestLcstatsBusy(false);
        }
    }

    async function copyLatestLcstatsPayload() {
        if (!latestLcstatsPayload) return;
        const text = JSON.stringify(latestLcstatsPayload.stats ?? latestLcstatsPayload.raw, null, 2);
        await navigator.clipboard.writeText(text);
        setLatestLcstatsCopied(true);
        window.setTimeout(() => setLatestLcstatsCopied(false), 1400);
    }

    async function applyThemeModeValue(nextMode) {
        const applied = await persistAndBroadcastThemeMode(nextMode);
        setThemeMode(applied);
    }

    async function refreshConfigLinkState() {
        try {
            const v = Number(selectedVersion);
            if (!Number.isFinite(v)) {
                setConfigLinkState(null);
                return;
            }
            const s = await invoke('get_config_link_state_for_version', { version: v });
            setConfigLinkState(s ?? null);
        } catch (e) {
            console.warn('Failed to read config link state', e);
            setConfigLinkState(null);
        }
    }

    useEffect(() => {
        refreshConfigLinkState();
    }, []);

    useEffect(() => {
        let unlisten = null;
        (async () => {
            unlisten = await listen('ui://selected-version-changed', (event) => {
                const v = Number(event?.payload?.version);
                if (!Number.isFinite(v)) return;
                setSelectedVersion(v);
            });
        })();
        return () => {
            if (typeof unlisten === 'function') unlisten();
        };
    }, []);

    useEffect(() => {
        if (!Number.isFinite(Number(selectedVersion))) return;
        refreshConfigLinkState();
    }, [selectedVersion]);

    useEffect(() => {
        if (!SHOW_DEV_SETTINGS || !settingsOpen || settingsTab !== "dev") return;
        refreshLatestLcstatsPayload();
        const interval = window.setInterval(() => {
            refreshLatestLcstatsPayload();
        }, 2000);
        return () => window.clearInterval(interval);
    }, [settingsOpen, settingsTab]);

    useEffect(() => {
        if (!settingsOpen || settingsTab !== "overlay") return;
        refreshOverlaySettings();
    }, [settingsOpen, settingsTab]);

    function handleSettingsOpenChange(open) {
        setSettingsOpen(open);
        if (!open) {
            void persistOverlaySettings(steamOverlayConfig, gameOverlayConfig, { showSaved: false });
            void invoke('close_obs_overlay_window_if_owned');
        }
    }

    async function persistOverlaySettings(
        nextSteamOverlayConfig = steamOverlayConfig,
        nextGameOverlayConfig = gameOverlayConfig,
        { showSaved = true } = {}
    ) {
        if (steamOverlayBusy) return;
        setSteamOverlayBusy(true);
        setSteamOverlayError("");
        if (showSaved) setSteamOverlaySaved("");
        try {
            const [saved, savedGameOverlay] = await Promise.all([
                invoke('set_steam_overlay_config', {
                    enabled: !!nextSteamOverlayConfig.enabled,
                    steamPath: nextSteamOverlayConfig.steam_path.trim() || null,
                }),
                invoke('set_game_overlay_config', {
                    config: {
                        ...nextGameOverlayConfig,
                        general: {
                            ...(nextGameOverlayConfig.general ?? {}),
                            enabled: nextGameOverlayConfig.general?.enabled !== false,
                        },
                    },
                }),
            ]);
            setSteamOverlayResolvedPath(String(saved?.resolved_steam_path ?? ""));
            setSteamOverlayConfig({
                enabled: !!saved?.enabled,
                steam_path: String(saved?.steam_path ?? saved?.resolved_steam_path ?? ""),
            });
            setGameOverlayConfig((prev) => ({
                ...prev,
                ...(savedGameOverlay ?? {}),
                general: {
                    ...(prev.general ?? {}),
                    ...(savedGameOverlay?.general ?? {}),
                    enabled: savedGameOverlay?.general?.enabled !== false,
                },
            }));
            if (showSaved) setSteamOverlaySaved("Saved");
        } catch (error) {
            setSteamOverlayError(error?.message ?? String(error));
        } finally {
            setSteamOverlayBusy(false);
        }
    }

    async function browseSteamOverlayPath() {
        if (steamOverlayBusy) return;
        setSteamOverlayError("");
        setSteamOverlaySaved("");
        try {
            const picked = await invoke('pick_steam_overlay_path', {
                initialPath: steamOverlayConfig.steam_path || steamOverlayResolvedPath || null,
            });
            if (!picked) return;
            setSteamOverlayConfig((prev) => ({
                ...prev,
                steam_path: String(picked),
            }));
            await persistOverlaySettings(
                { ...steamOverlayConfig, steam_path: String(picked) },
                gameOverlayConfig,
                { showSaved: true }
            );
        } catch (error) {
            setSteamOverlayError(error?.message ?? String(error));
        }
    }

    async function openObsOverlayWindow() {
        if (obsOverlayBusy) return;
        setObsOverlayBusy(true);
        setSteamOverlayError("");
        setSteamOverlaySaved("");
        try {
            await invoke('open_obs_overlay_window');
            setSteamOverlaySaved("OBS selector requested. Select HQ Overlay - OBS Capture in OBS, then hide the selector.");
        } catch (error) {
            setSteamOverlayError(error?.message ?? String(error));
        } finally {
            setObsOverlayBusy(false);
        }
    }

    // Fix: after installing/downloading a version, the folder appears but `selectedVersion`
    // may not change, so refresh link state when that install finishes.
    useEffect(() => {
        if (!Number.isFinite(Number(selectedVersion))) return;
        let unlisten = null;
        (async () => {
            unlisten = await listen('download://finished', (event) => {
                const v = Number(event?.payload?.version);
                if (!Number.isFinite(v)) return;
                if (v !== Number(selectedVersion)) return;
                refreshConfigLinkState();
            });
        })();
        return () => {
            if (typeof unlisten === 'function') unlisten();
        };
    }, [selectedVersion]);

    const canManageConfigLink =
        Number.isFinite(Number(selectedVersion)) && !!configLinkState?.is_installed;
    const unlinkDisabled = configLinkBusy || !canManageConfigLink || !configLinkState?.is_linked;
    const linkDisabled = configLinkBusy || !canManageConfigLink || !!configLinkState?.is_linked;

    async function unlinkConfig() {
        if (unlinkDisabled) return;
        setConfigLinkBusy(true);
        try {
            const s = await invoke('unlink_config_for_version', { version: selectedVersion });
            setConfigLinkState(s ?? null);
            await emit('config://link-changed', { version: selectedVersion, state: s ?? null });
        } catch (e) {
            window.alert(e?.message ?? String(e));
        } finally {
            setConfigLinkBusy(false);
        }
    }

    async function linkConfig() {
        if (linkDisabled) return;

        // If currently unlinked, show a proper modal (better UX + compatibility than window.confirm).
        if (configLinkState && !configLinkState.is_linked) {
            setLinkWarnOpen(true);
            return;
        }

        await doLinkConfig();
    }

    async function doLinkConfig() {
        if (linkDisabled) return;
        setConfigLinkBusy(true);
        try {
            const s = await invoke('link_config_for_version', { version: selectedVersion });
            setConfigLinkState(s ?? null);
            await emit('config://link-changed', { version: selectedVersion, state: s ?? null });
        } catch (e) {
            window.alert(e?.message ?? String(e));
        } finally {
            setConfigLinkBusy(false);
        }
    }

    const fileMenuItems = useMemo(() => ([
        { label: "Open Version Folder", shortcut: "CommandOrControl+O", action: () => { invoke('open_version_folder'); } },
        { label: "Open DepotDownloader Folder", shortcut: "", action: () => { invoke('open_downloader_folder'); } },
        { label: "Open Overlay Modules Folder", shortcut: "", action: () => { invoke('open_game_overlay_modules_folder'); } }
    ]), []);

    const editMenuItems = useMemo(() => ([
        { label: "Launch Options", shortcut: "", action: () => { emit('ui://open-launch-options'); } },
        { label: "Unlink Config", shortcut: "", disabled: unlinkDisabled, action: unlinkConfig },
        { label: "Link Config", shortcut: "", disabled: linkDisabled, action: linkConfig }
    ]), [linkDisabled, unlinkDisabled, configLinkState, configLinkBusy, selectedVersion]);


    return (
        <>
            <Dialog open={linkWarnOpen} onOpenChange={setLinkWarnOpen}>
                <DialogContent
                    onEscapeKeyDown={(e) => {
                        if (configLinkBusy) e.preventDefault();
                    }}
                    onPointerDownOutside={(e) => {
                        if (configLinkBusy) e.preventDefault();
                    }}
                >
                    <div className="flex flex-col gap-4">
                        <div>
                            <div className="text-lg font-semibold">Link Config</div>
                            <div className="mt-1 text-sm text-white/55 whitespace-pre-wrap">
                                Linking config will switch the game to the shared config folder.
                                {"\n\n"}
                                Changes made while unlinked may not be reflected after linking.
                            </div>
                        </div>

                        <div className="flex items-center justify-end gap-2">
                            {!configLinkBusy && (
                                <Button
                                    variant="outline"
                                    className="h-10"
                                    onClick={() => setLinkWarnOpen(false)}
                                >
                                    Cancel
                                </Button>
                            )}
                            <Button
                                variant="default"
                                className="h-10"
                                disabled={configLinkBusy}
                                onClick={async () => {
                                    setLinkWarnOpen(false);
                                    await doLinkConfig();
                                }}
                            >
                                Continue
                            </Button>
                        </div>
                    </div>
                </DialogContent>
            </Dialog>

            <Dialog open={settingsOpen} onOpenChange={handleSettingsOpenChange}>
                <DialogContent className="w-[min(760px,94vw)] overflow-hidden rounded-2xl p-0">
                    <div className="flex h-[min(560px,78vh)] min-h-[420px] flex-col">
                        <div className="flex h-11 shrink-0 items-center justify-between border-b border-panel-outline px-4">
                            <div className="flex items-center gap-2 text-sm font-semibold text-white">
                                <Settings size={15} className="text-white/65" />
                                Settings
                            </div>
                            <button
                                data-tauri-drag-region="false"
                                className="flex h-7 w-7 cursor-pointer items-center justify-center rounded-sm bg-transparent p-0 text-white/70 shadow-none hover:bg-white/[0.07] hover:text-white"
                                onClick={() => handleSettingsOpenChange(false)}
                                aria-label="Close settings"
                            >
                                <X size={16} />
                            </button>
                        </div>

                        <div className="grid min-h-0 flex-1 grid-cols-[230px_minmax(0,1fr)]">
                            <aside className="border-r border-panel-outline p-4">
                                <div className="flex flex-col gap-2">
                                    <button
                                        className={cn(
                                            "relative flex h-11 items-center gap-3 rounded-md px-3 text-left text-sm font-semibold shadow-none",
                                            settingsTab === "general"
                                                ? "border border-white/10 bg-white/[0.06] text-white"
                                                : "bg-transparent text-white/55 hover:bg-white/5 hover:text-white/80"
                                        )}
                                        onClick={() => setSettingsTab("general")}
                                    >
                                        {settingsTab === "general" && (
                                            <span className="absolute left-0 top-2 h-7 w-0.5 rounded-r bg-[var(--theme-accent)]" />
                                        )}
                                        <span className="flex h-7 w-7 items-center justify-center rounded-md bg-black/20 text-white/80">
                                            <Settings size={16} />
                                        </span>
                                        General
                                    </button>
                                    <button
                                        className={cn(
                                            "relative flex h-11 items-center gap-3 rounded-md px-3 text-left text-sm font-semibold shadow-none",
                                            settingsTab === "overlay"
                                                ? "border border-white/10 bg-white/[0.06] text-white"
                                                : "bg-transparent text-white/55 hover:bg-white/5 hover:text-white/80"
                                        )}
                                        onClick={() => setSettingsTab("overlay")}
                                    >
                                        {settingsTab === "overlay" && (
                                            <span className="absolute left-0 top-2 h-7 w-0.5 rounded-r bg-[var(--theme-accent)]" />
                                        )}
                                        <span className="flex h-7 w-7 items-center justify-center rounded-md bg-white/5 text-white/55">
                                            <Play size={16} />
                                        </span>
                                        Overlay
                                    </button>
                                    {SHOW_THEME_SETTINGS && (
                                        <button
                                            className={cn(
                                                "relative flex h-11 items-center gap-3 rounded-md px-3 text-left text-sm font-semibold shadow-none",
                                                settingsTab === "theme"
                                                    ? "border border-white/10 bg-white/[0.06] text-white"
                                                    : "bg-transparent text-white/55 hover:bg-white/5 hover:text-white/80"
                                            )}
                                            onClick={() => setSettingsTab("theme")}
                                        >
                                            {settingsTab === "theme" && (
                                                <span className="absolute left-0 top-2 h-7 w-0.5 rounded-r bg-[var(--theme-accent)]" />
                                            )}
                                            <span className="flex h-7 w-7 items-center justify-center rounded-md bg-white/5 text-white/55">
                                                <Paintbrush size={16} />
                                            </span>
                                            Theme
                                        </button>
                                    )}
                                    {SHOW_DEV_SETTINGS && (
                                        <button
                                            className={cn(
                                                "relative flex h-11 items-center gap-3 rounded-md px-3 text-left text-sm font-semibold shadow-none",
                                                settingsTab === "dev"
                                                    ? "border border-white/10 bg-white/[0.06] text-white"
                                                    : "bg-transparent text-white/55 hover:bg-white/5 hover:text-white/80"
                                            )}
                                            onClick={() => setSettingsTab("dev")}
                                        >
                                            {settingsTab === "dev" && (
                                                <span className="absolute left-0 top-2 h-7 w-0.5 rounded-r bg-[var(--theme-accent)]" />
                                            )}
                                            <span className="flex h-7 w-7 items-center justify-center rounded-md bg-white/5 text-white/55">
                                                <Beaker size={16} />
                                            </span>
                                            Dev
                                        </button>
                                    )}
                                </div>
                            </aside>

                            <section className="min-w-0 overflow-y-auto p-5">
                                {settingsTab === "general" && (
                                    <div className="space-y-4">
                                        <div>
                                            <div className="text-base font-semibold text-white">Release Channel</div>
                                            <div className="mt-1 text-sm text-white/50">
                                                Choose which launcher updates and remote manifest this app follows.
                                            </div>
                                        </div>

                                        <div className="rounded-lg border border-panel-outline p-4">
                                            <div className="flex items-start justify-between gap-4">
                                                <div className="flex min-w-0 gap-3">
                                                    <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-md border border-white/10 bg-black/20 text-white/75">
                                                        <Beaker size={18} />
                                                    </div>
                                                    <div className="min-w-0">
                                                        <div className="text-sm font-semibold text-white">Beta channel</div>
                                                        <div className="mt-1 text-sm leading-5 text-white/55">
                                                            Get faster and more frequent updates before they reach stable.
                                                        </div>
                                                    </div>
                                                </div>

                                                <Switch
                                                    checked={!!releaseChannel?.is_beta}
                                                    disabled={releaseChannelBusy || !releaseChannel}
                                                    onCheckedChange={setBetaEnabled}
                                                    aria-label="Enable beta channel"
                                                />
                                            </div>

                                            {releaseChannelError && (
                                                <div className="mt-3 rounded-md border border-red-400/30 bg-red-500/10 px-3 py-2 text-sm text-red-100">
                                                    {releaseChannelError}
                                                </div>
                                            )}
                                        </div>

                                        <div className="rounded-lg border border-panel-outline p-4">
                                            <div className="flex items-start justify-between gap-4">
                                                <div className="flex min-w-0 gap-3">
                                                    <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-md border border-white/10 bg-black/20 text-white/75">
                                                        <HardDrive size={18} />
                                                    </div>
                                                    <div className="min-w-0">
                                                        <div className="text-sm font-semibold text-white">Game storage</div>
                                                        <div className="mt-1 text-sm leading-5 text-white/55">
                                                            Installed game versions are stored here.
                                                        </div>
                                                    </div>
                                                </div>
                                                {gameStorage?.is_custom && (
                                                    <Button
                                                        variant="outline"
                                                        className="h-9 shrink-0 px-3"
                                                        disabled={gameStorageBusy}
                                                        onClick={() => {
                                                            void resetGameStorageDir();
                                                        }}
                                                    >
                                                        <RotateCcw className="h-4 w-4" />
                                                        Reset
                                                    </Button>
                                                )}
                                            </div>

                                            <div className="mt-4 flex items-center gap-2 rounded-md border border-panel-outline bg-black/20 px-3 py-2">
                                                <div
                                                    className="min-w-0 flex-1 truncate text-xs font-medium text-white/65"
                                                    title={gameStorage?.current_dir ?? ""}
                                                >
                                                    {gameStorage?.current_dir ?? "Loading..."}
                                                </div>
                                                <Button
                                                    variant="outline"
                                                    className="h-8 shrink-0 px-3 text-xs"
                                                    disabled={gameStorageBusy}
                                                    onClick={() => {
                                                        void changeGameStorageDir();
                                                    }}
                                                >
                                                    <FolderOpen className="h-4 w-4" />
                                                    Change
                                                </Button>
                                            </div>

                                            {gameStorageError && (
                                                <div className="mt-3 rounded-md border border-red-400/30 bg-red-500/10 px-3 py-2 text-sm text-red-100">
                                                    {gameStorageError}
                                                </div>
                                            )}
                                        </div>
                                    </div>
                                )}

                                {settingsTab === "overlay" && (
                                    <div className="space-y-4">
                                        <div className="flex items-start justify-between gap-4">
                                            <div>
                                                <div className="text-base font-semibold text-white">Overlay</div>
                                                <div className="mt-1 text-sm text-white/50">
                                                    Configure the HQLC in-game overlay and OBS selector.
                                                </div>
                                            </div>
                                        </div>

                                        <div className="rounded-lg border border-panel-outline p-4">
                                            <div className="flex items-start justify-between gap-4">
                                                <div className="min-w-0">
                                                    <div className="text-sm font-semibold text-white">Enable HQLC Overlay</div>
                                                    <div className="mt-1 text-sm leading-5 text-white/55">
                                                        Shows the editable HQLC in-game overlay while Lethal Company is focused.
                                                    </div>
                                                </div>
                                                <Switch
                                                    checked={gameOverlayConfig.general?.enabled !== false}
                                                    disabled={steamOverlayBusy}
                                                    onCheckedChange={(checked) => {
                                                        const nextGameOverlayConfig = {
                                                            ...gameOverlayConfig,
                                                            general: {
                                                                ...(gameOverlayConfig.general ?? {}),
                                                                enabled: checked,
                                                            },
                                                        };
                                                        setGameOverlayConfig(nextGameOverlayConfig);
                                                        setSteamOverlaySaved("");
                                                        void persistOverlaySettings(steamOverlayConfig, nextGameOverlayConfig);
                                                    }}
                                                    aria-label="Enable HQLC overlay"
                                                />
                                            </div>
                                        </div>

                                        <div className="rounded-lg border border-panel-outline p-4">
                                            <div className="flex items-start justify-between gap-4">
                                                <div className="min-w-0">
                                                    <div className="text-sm font-semibold text-white">Use StreamOverlays API</div>
                                                    <div className="mt-1 text-sm leading-5 text-white/55">
                                                        Lets HQLC overlay modules read StreamOverlays data from its local WebSocket.
                                                    </div>
                                                </div>
                                                <Switch
                                                    checked={gameOverlayConfig.general?.use_stream_overlays_api === true}
                                                    disabled={steamOverlayBusy}
                                                    onCheckedChange={(checked) => {
                                                        const nextGameOverlayConfig = {
                                                            ...gameOverlayConfig,
                                                            general: {
                                                                ...(gameOverlayConfig.general ?? {}),
                                                                use_stream_overlays_api: checked,
                                                            },
                                                        };
                                                        setGameOverlayConfig(nextGameOverlayConfig);
                                                        setSteamOverlaySaved("");
                                                        void persistOverlaySettings(steamOverlayConfig, nextGameOverlayConfig);
                                                    }}
                                                    aria-label="Use StreamOverlays API"
                                                />
                                            </div>
                                        </div>

                                        <div className="rounded-lg border border-panel-outline p-4">
                                            <div className="flex items-start justify-between gap-4">
                                                <div className="min-w-0">
                                                    <div className="text-sm font-semibold text-white">OBS Capture Window</div>
                                                    <div className="mt-1 text-sm leading-5 text-white/55">
                                                        Shows the game overlay window for OBS selection. Closing settings will hide temporary selector overlays.
                                                    </div>
                                                </div>
                                                <div className="flex shrink-0 items-center gap-2">
                                                    <Button
                                                        variant="default"
                                                        className="h-9 px-3"
                                                        disabled={steamOverlayBusy || obsOverlayBusy}
                                                        onClick={() => {
                                                            void openObsOverlayWindow();
                                                        }}
                                                    >
                                                        <Play className="h-4 w-4" />
                                                        {obsOverlayBusy ? "Opening..." : "Open Selector"}
                                                    </Button>
                                                </div>
                                            </div>
                                        </div>

                                        <div className="rounded-lg border border-panel-outline p-4">
                                            <div className="flex items-start justify-between gap-4">
                                                <div className="min-w-0">
                                                    <div className="text-sm font-semibold text-white">Enable Inject Steam Overlay</div>
                                                    <div className="mt-1 text-sm leading-5 text-white/55">
                                                        Starts the game suspended, injects Steam overlay DLLs, then resumes the process.
                                                    </div>
                                                </div>
                                                <Switch
                                                    checked={steamOverlayConfig.enabled}
                                                    disabled={steamOverlayBusy}
                                                    onCheckedChange={(checked) => {
                                                        const nextSteamOverlayConfig = {
                                                            ...steamOverlayConfig,
                                                            enabled: checked,
                                                        };
                                                        setSteamOverlayConfig(nextSteamOverlayConfig);
                                                        setSteamOverlaySaved("");
                                                        void persistOverlaySettings(nextSteamOverlayConfig, gameOverlayConfig);
                                                    }}
                                                    aria-label="Enable inject Steam overlay"
                                                />
                                            </div>

                                            <div className="mt-4">
                                                <label className="mb-2 block text-sm font-semibold text-white/80" htmlFor="settings-steam-overlay-path">
                                                    Steam path override
                                                </label>
                                                <div className="flex items-center gap-2">
                                                    <input
                                                        id="settings-steam-overlay-path"
                                                        value={steamOverlayConfig.steam_path}
                                                        disabled={steamOverlayBusy}
                                                        onBlur={() => {
                                                            void persistOverlaySettings(steamOverlayConfig, gameOverlayConfig);
                                                        }}
                                                        onChange={(event) => {
                                                            setSteamOverlayConfig((prev) => ({
                                                                ...prev,
                                                                steam_path: event.target.value,
                                                            }));
                                                            setSteamOverlaySaved("");
                                                        }}
                                                        placeholder="Leave blank to auto-detect Steam"
                                                        className="h-10 min-w-0 flex-1 rounded-md border border-panel-outline bg-black/20 px-3 text-sm text-white outline-none placeholder:text-white/35 focus:ring-2 focus:ring-panel-outline disabled:cursor-not-allowed disabled:opacity-60"
                                                    />
                                                    <Button
                                                        variant="outline"
                                                        className="h-10 shrink-0 px-3"
                                                        disabled={steamOverlayBusy}
                                                        onClick={() => {
                                                            void browseSteamOverlayPath();
                                                        }}
                                                    >
                                                        <FolderOpen className="h-4 w-4" />
                                                        Browse
                                                    </Button>
                                                </div>
                                                <div className="mt-2 text-xs leading-5 text-white/45">
                                                    {steamOverlayResolvedPath ? (
                                                        <>
                                                            Auto-detected path: <span className="font-mono">{steamOverlayResolvedPath}</span>
                                                        </>
                                                    ) : (
                                                        <>
                                                            Example: <span className="font-mono">C:\Program Files (x86)\Steam</span>
                                                        </>
                                                    )}
                                                </div>
                                            </div>
                                        </div>

                                        {steamOverlayError && (
                                            <div className="rounded-md border border-red-400/30 bg-red-500/10 px-3 py-2 text-sm text-red-100">
                                                {steamOverlayError}
                                            </div>
                                        )}

                                        {steamOverlaySaved && (
                                            <div className="rounded-md border border-[color-mix(in_srgb,var(--theme-accent)_35%,transparent)] bg-[var(--theme-accent-muted)] px-3 py-2 text-sm text-[var(--theme-accent)]">
                                                {steamOverlaySaved}
                                            </div>
                                        )}
                                    </div>
                                )}

                                {settingsTab === "theme" && (
                                    <div className="space-y-4">
                                        <div className="flex items-start justify-between gap-4">
                                            <div>
                                                <div className="text-base font-semibold text-white">Theme</div>
                                                <div className="mt-1 text-sm text-white/50">
                                                    Adjust the launcher accent color.
                                                </div>
                                            </div>
                                            <Button
                                                variant="outline"
                                                className="h-9 shrink-0"
                                                onClick={() => {
                                                    void applyThemeHueValue(DEFAULT_THEME_HUE);
                                                    void applyThemeBrightnessValue(DEFAULT_THEME_BRIGHTNESS);
                                                    void applyThemeModeValue(DEFAULT_THEME_MODE);
                                                }}
                                            >
                                                <RotateCcw className="h-4 w-4" />
                                                Reset
                                            </Button>
                                        </div>

                                        <div className="rounded-lg border border-panel-outline p-4">
                                            <div className="space-y-5">
                                                <div className="space-y-3">
                                                    <div className="text-sm font-semibold text-white">Mode</div>
                                                    <div className="grid grid-cols-2 gap-2">
                                                        {[
                                                            { value: "dark", label: "Dark", icon: Moon },
                                                            { value: "light", label: "Light", icon: Sun },
                                                        ].map((option) => {
                                                            const Icon = option.icon;
                                                            const selected = normalizedThemeMode === option.value;
                                                            return (
                                                                <button
                                                                    key={option.value}
                                                                    type="button"
                                                                    className={cn(
                                                                        "flex h-11 items-center justify-center gap-2 rounded-lg border text-sm font-semibold transition-colors",
                                                                        selected
                                                                            ? "border-[color-mix(in_srgb,var(--theme-accent)_55%,transparent)] bg-[var(--theme-accent-muted)] text-[var(--theme-accent)]"
                                                                            : "border-panel-outline bg-transparent text-white/65 hover:bg-white/[0.07] hover:text-white"
                                                                    )}
                                                                    onClick={() => {
                                                                        setThemeMode(option.value);
                                                                        void applyThemeModeValue(option.value);
                                                                    }}
                                                                    aria-pressed={selected}
                                                                >
                                                                    <Icon className="h-4 w-4" />
                                                                    {option.label}
                                                                </button>
                                                            );
                                                        })}
                                                    </div>
                                                </div>

                                                <div className="flex items-center justify-between gap-3">
                                                    <div className="text-sm font-semibold text-white">Theme hue</div>
                                                    <input
                                                        type="number"
                                                        min="0"
                                                        max="360"
                                                        step="1"
                                                        value={normalizedThemeHue}
                                                        onChange={(event) => {
                                                            const nextHue = normalizeThemeHue(event.target.value);
                                                            setThemeHue(nextHue);
                                                            void applyThemeHueValue(nextHue);
                                                        }}
                                                        className="h-8 w-16 rounded-md border border-panel-outline bg-black/20 px-2 text-center text-sm font-semibold text-[var(--theme-accent)] outline-none focus:ring-2 focus:ring-panel-outline"
                                                        aria-label="Theme hue value"
                                                    />
                                                </div>

                                                <div className="relative h-7 rounded-md border border-white/10 p-1">
                                                    <div
                                                        className="absolute inset-1 rounded"
                                                        style={{
                                                            background:
                                                                "linear-gradient(90deg, hsl(0 86% 64%), hsl(60 86% 64%), hsl(120 86% 64%), hsl(180 86% 64%), hsl(240 86% 64%), hsl(300 86% 64%), hsl(360 86% 64%))",
                                                        }}
                                                    />
                                                    <input
                                                        type="range"
                                                        min="0"
                                                        max="360"
                                                        step="1"
                                                        value={normalizedThemeHue}
                                                        onChange={(event) => {
                                                            const nextHue = normalizeThemeHue(event.target.value);
                                                            setThemeHue(nextHue);
                                                            void applyThemeHueValue(nextHue);
                                                        }}
                                                        className="theme-hue-slider absolute inset-0 h-full w-full cursor-pointer appearance-none bg-transparent"
                                                        aria-label="Theme hue"
                                                    />
                                                </div>

                                                <div className="space-y-3">
                                                    <div className="flex items-center justify-between gap-3">
                                                        <div className="text-sm font-semibold text-white">Brightness</div>
                                                        <input
                                                            type="number"
                                                            min="0"
                                                            max="100"
                                                            step="1"
                                                            value={normalizedThemeBrightness}
                                                            onChange={(event) => {
                                                                const nextBrightness = normalizeThemeBrightness(event.target.value);
                                                                setThemeBrightness(nextBrightness);
                                                                void applyThemeBrightnessValue(nextBrightness);
                                                            }}
                                                            className="h-8 w-16 rounded-md border border-panel-outline bg-black/20 px-2 text-center text-sm font-semibold text-white/70 outline-none focus:ring-2 focus:ring-panel-outline"
                                                            aria-label="Theme brightness value"
                                                        />
                                                    </div>

                                                    <div className="relative h-7 rounded-md border border-white/10 p-1">
                                                        <div
                                                            className={cn(
                                                                "absolute inset-1 rounded",
                                                                normalizedThemeMode === "light"
                                                                    ? "bg-gradient-to-r from-white via-[#d8dde3] to-[#7b8794]"
                                                                    : "bg-gradient-to-r from-black via-[#30343d] to-white"
                                                            )}
                                                        />
                                                        <input
                                                            type="range"
                                                            min="0"
                                                            max="100"
                                                            step="1"
                                                            value={normalizedThemeBrightness}
                                                            onChange={(event) => {
                                                                const nextBrightness = normalizeThemeBrightness(event.target.value);
                                                                setThemeBrightness(nextBrightness);
                                                                void applyThemeBrightnessValue(nextBrightness);
                                                            }}
                                                            className="theme-hue-slider absolute inset-0 h-full w-full cursor-pointer appearance-none bg-transparent"
                                                            aria-label="Theme brightness"
                                                        />
                                                    </div>
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                )}

                                {settingsTab === "dev" && SHOW_DEV_SETTINGS && (
                                    <div className="space-y-4">
                                        <div className="flex items-start justify-between gap-4">
                                            <div>
                                                <div className="text-base font-semibold text-white">Dev</div>
                                                <div className="mt-1 text-sm text-white/50">
                                                    Latest LCStatsTracker payload captured by AutoSheet.
                                                </div>
                                            </div>
                                            <Button
                                                variant="outline"
                                                className="h-9 shrink-0"
                                                disabled={latestLcstatsBusy}
                                                onClick={() => {
                                                    void refreshLatestLcstatsPayload();
                                                }}
                                            >
                                                <RefreshCw className={cn("h-4 w-4", latestLcstatsBusy ? "animate-spin" : "")} />
                                                Refresh
                                            </Button>
                                        </div>

                                        <div className="rounded-lg border border-panel-outline p-4">
                                            <div className="flex items-center justify-between gap-3">
                                                <div className="min-w-0">
                                                    <div className="text-sm font-semibold text-white">LCStatsTracker</div>
                                                    <div className="mt-1 text-xs text-white/45">
                                                        {latestLcstatsPayload?.receivedAt
                                                            ? new Date(latestLcstatsPayload.receivedAt * 1000).toLocaleString()
                                                            : "No payload captured yet"}
                                                    </div>
                                                </div>
                                                <Button
                                                    variant="outline"
                                                    className="h-8 shrink-0 px-3 text-xs"
                                                    disabled={!latestLcstatsPayload}
                                                    onClick={() => {
                                                        void copyLatestLcstatsPayload();
                                                    }}
                                                >
                                                    <Copy className="h-4 w-4" />
                                                    {latestLcstatsCopied ? "Copied" : "Copy"}
                                                </Button>
                                            </div>

                                            <pre className="mt-4 max-h-72 overflow-auto rounded-md border border-panel-outline bg-black/30 p-3 text-xs leading-5 text-white/70">
                                                {latestLcstatsPayload
                                                    ? JSON.stringify(latestLcstatsPayload.stats ?? latestLcstatsPayload.raw, null, 2)
                                                    : "Waiting for LCStatsTracker payload..."}
                                            </pre>

                                            {latestLcstatsError && (
                                                <div className="mt-3 rounded-md border border-red-400/30 bg-red-500/10 px-3 py-2 text-sm text-red-100">
                                                    {latestLcstatsError}
                                                </div>
                                            )}
                                        </div>
                                    </div>
                                )}
                            </section>
                        </div>
                    </div>
                </DialogContent>
            </Dialog>

            <div 
                data-tauri-drag-region 
                className={cn('w-full flex items-center justify-between px-2 border-b border-panel-outline bg-[color-mix(in_srgb,var(--theme-bg)_84%,transparent)] z-50', props.className)}
            >
                {/* Left side - Menu items */}
                <div className="flex items-center gap-1">
                    <button
                        data-tauri-drag-region="false"
                        className="ml-1 flex h-8 w-8 cursor-pointer items-center justify-center rounded-sm bg-transparent p-0 shadow-none hover:bg-white/[0.07]"
                        onClick={() => handleSettingsOpenChange(true)}
                        aria-label="Open settings"
                    >
                        <img src="/icon.svg" alt="logo" className='h-6 w-6' />
                    </button>
                    <TitlebarMenu 
                        name="File" 
                        items={fileMenuItems} 
                    />
                    <TitlebarMenu 
                        name="Edit" 
                        items={editMenuItems} 
                    />
                </div>

                {/* Right side - Window controls */}
                <div className="flex items-center" data-tauri-drag-region="false">
                    <button
                        onClick={handleMinimize}
                        className="w-8 h-8 flex cursor-pointer items-center justify-center hover:bg-white/[0.07] transition-colors rounded-sm"
                        aria-label="Minimize"
                    >
                        <Minus size={14} className="text-white/80" />
                    </button>
                    <button
                        onClick={handleMaximize}
                        className="w-8 h-8 flex cursor-pointer items-center justify-center hover:bg-white/[0.07] transition-colors rounded-sm"
                        aria-label={isMaximized ? "Restore" : "Maximize"}
                    >
                        <Square size={12} className="text-white/80" />
                    </button>
                    <button
                        onClick={handleClose}
                        className="w-8 h-8 flex cursor-pointer items-center justify-center hover:bg-red-500/20 transition-colors rounded-sm"
                        aria-label="Close"
                    >
                        <X size={14} className="text-white/80" />
                    </button>
                </div>
            </div>
        </>
    );
}

function TitlebarMenu({ name, items }) {
    const formatShortcut = (shortcut) =>
        String(shortcut ?? "").replaceAll("CommandOrControl", "Ctrl");


    useEffect(() => {
        let disposed = false;
        const shortcuts = [...new Set(
            items
                .map((item) => String(item.shortcut ?? "").trim())
                .filter(Boolean)
        )];

        (async () => {
            for (const shortcut of shortcuts) {
                if (disposed) return;
                try {
                    if (await isRegistered(shortcut)) {
                        await unregister(shortcut);
                    }
                    await register(shortcut, (event) => {
                        if (event.state !== "Pressed") return;
                        const item = items.find((entry) => entry.shortcut === shortcut);
                        if (item?.disabled) return;
                        item?.action?.();
                    });
                } catch (error) {
                    console.warn(`Failed to register shortcut: ${shortcut}`, error);
                }
            }
        })();

        return () => {
            disposed = true;
            if (shortcuts.length > 0) {
                unregister(shortcuts).catch(() => {});
            }
        };
    }, [items]);
    return (
        <DropdownMenu.Root>
            <DropdownMenu.Trigger asChild>
                <button
                    data-tauri-drag-region="false"
                    className={cn(
                        "cursor-pointer text-xs font-light text-white/90 px-2 py-1 hover:bg-white/[0.07] rounded transition-colors outline-none",
                        "data-[state=open]:bg-white/[0.08]"
                    )}
                >
                    {name}
                </button>
            </DropdownMenu.Trigger>

            <DropdownMenu.Portal>
                <DropdownMenu.Content
                    className={cn(
                        "min-w-[200px] bg-[var(--theme-surface)] border border-panel-outline rounded-md shadow-lg py-1 z-50",
                        "data-[state=open]:animate-in data-[state=closed]:animate-out",
                        "data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0",
                        "data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95",
                        "data-[side=bottom]:slide-in-from-top-2"
                    )}
                    sideOffset={4}
                    align="start"
                >
                    {items.map((item, index) => (
                        (() => {
                            const disabled = !!item.disabled;
                            return (
                        <DropdownMenu.Item
                            key={index}
                            disabled={disabled}
                            onSelect={() => {
                                if (disabled) { return; }
                                item.action?.();
                            }}
                            className={cn(
                                "w-full text-left text-xs text-white/90 px-3 py-1.5",
                                disabled
                                    ? "opacity-40 cursor-not-allowed"
                                    : "hover:bg-white/[0.07] transition-colors cursor-pointer",
                                "outline-none",
                                "flex items-center justify-between",
                                "focus:bg-white/[0.07]"
                            )}
                        >
                            <span>{item.label}</span>
                            {item.shortcut && (
                                <span className="text-xs text-white/50 ml-4">{formatShortcut(item.shortcut)}</span>
                            )}
                        </DropdownMenu.Item>
                            );
                        })()
                    ))}
                </DropdownMenu.Content>
            </DropdownMenu.Portal>
        </DropdownMenu.Root>
    );
}
