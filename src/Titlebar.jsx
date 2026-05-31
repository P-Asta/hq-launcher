import { getCurrentWindow } from '@tauri-apps/api/window';
import { useEffect, useMemo, useState } from 'react';
import { Beaker, Info, Minus, Settings, Square, X } from 'lucide-react';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import { cn } from './lib/cn';
import { invoke } from '@tauri-apps/api/core';
import { emit, listen } from '@tauri-apps/api/event';
import { isRegistered, register, unregister, unregisterAll } from '@tauri-apps/plugin-global-shortcut';
import { Dialog, DialogContent } from './components/ui/dialog';
import { Button } from './components/ui/button';
import { Switch } from './components/ui/switch';


export default function Titlebar({ installedVersions, ...props }) {
    const [isMaximized, setIsMaximized] = useState(false);
    const [configLinkState, setConfigLinkState] = useState(null);
    const [configLinkBusy, setConfigLinkBusy] = useState(false);
    const [linkWarnOpen, setLinkWarnOpen] = useState(false);
    const [settingsOpen, setSettingsOpen] = useState(false);
    const [releaseChannel, setReleaseChannel] = useState(null);
    const [releaseChannelBusy, setReleaseChannelBusy] = useState(false);
    const [releaseChannelError, setReleaseChannelError] = useState("");
    const [selectedVersion, setSelectedVersion] = useState(null);
    
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
    }, []);

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
        { label: "Open DepotDownloader Folder", shortcut: "", action: () => { invoke('open_downloader_folder'); } }
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

            <Dialog open={settingsOpen} onOpenChange={setSettingsOpen}>
                <DialogContent className="w-[min(760px,94vw)] overflow-hidden rounded-2xl p-0">
                    <div className="flex h-[min(560px,78vh)] min-h-[420px] flex-col">
                        <div className="flex h-11 shrink-0 items-center justify-between border-b border-panel-outline bg-[#0b0c10]/95 px-4">
                            <div className="flex items-center gap-2 text-sm font-semibold text-white">
                                <Settings size={15} className="text-white/65" />
                                Settings
                            </div>
                            <button
                                data-tauri-drag-region="false"
                                className="flex h-7 w-7 items-center justify-center rounded-sm bg-transparent p-0 text-white/70 shadow-none hover:bg-white/10 hover:text-white"
                                onClick={() => setSettingsOpen(false)}
                                aria-label="Close settings"
                            >
                                <X size={16} />
                            </button>
                        </div>

                        <div className="grid min-h-0 flex-1 grid-cols-[230px_minmax(0,1fr)] bg-[#0f1116]">
                            <aside className="border-r border-panel-outline bg-white/[0.02] p-4">
                                <div className="flex flex-col gap-2">
                                    <button className="relative flex h-11 items-center gap-3 rounded-md border border-white/10 bg-white/[0.08] px-3 text-left text-sm font-semibold text-white shadow-none">
                                        <span className="absolute left-0 top-2 h-7 w-0.5 rounded-r bg-white/60" />
                                        <span className="flex h-7 w-7 items-center justify-center rounded-md bg-white/10 text-white/80">
                                            <Settings size={16} />
                                        </span>
                                        General
                                    </button>
                                    <button className="flex h-11 items-center gap-3 rounded-md bg-transparent px-3 text-left text-sm font-semibold text-white/55 shadow-none hover:bg-white/5 hover:text-white/80">
                                        <span className="flex h-7 w-7 items-center justify-center rounded-md bg-white/5 text-white/55">
                                            <Info size={16} />
                                        </span>
                                        About
                                    </button>
                                </div>
                            </aside>

                            <section className="min-w-0 overflow-y-auto p-5">
                                <div className="space-y-4">
                                    <div>
                                        <div className="text-base font-semibold text-white">Release Channel</div>
                                        <div className="mt-1 text-sm text-white/50">
                                            Choose which launcher updates and remote manifest this app follows.
                                        </div>
                                    </div>

                                    <div className="rounded-lg border border-panel-outline bg-white/[0.03] p-4 shadow-lg shadow-black/20">
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

                                </div>
                            </section>
                        </div>
                    </div>
                </DialogContent>
            </Dialog>

            <div 
                data-tauri-drag-region 
                className={cn('w-full flex items-center justify-between px-2 border-b border-panel-outline bg-[#0b0c10]/80 backdrop-blur-sm z-50', props.className)}
            >
                {/* Left side - Menu items */}
                <div className="flex items-center gap-1">
                    <button
                        data-tauri-drag-region="false"
                        className="ml-1 flex h-8 w-8 items-center justify-center rounded-sm bg-transparent p-0 shadow-none hover:bg-white/10"
                        onClick={() => setSettingsOpen(true)}
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
                        className="w-8 h-8 flex items-center justify-center hover:bg-white/10 transition-colors rounded-sm"
                        aria-label="Minimize"
                    >
                        <Minus size={14} className="text-white/80" />
                    </button>
                    <button
                        onClick={handleMaximize}
                        className="w-8 h-8 flex items-center justify-center hover:bg-white/10 transition-colors rounded-sm"
                        aria-label={isMaximized ? "Restore" : "Maximize"}
                    >
                        <Square size={12} className="text-white/80" />
                    </button>
                    <button
                        onClick={handleClose}
                        className="w-8 h-8 flex items-center justify-center hover:bg-red-500/20 transition-colors rounded-sm"
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
                        "text-xs font-light text-white/90 px-2 py-1 hover:bg-white/10 rounded transition-colors outline-none",
                        "data-[state=open]:bg-white/10"
                    )}
                >
                    {name}
                </button>
            </DropdownMenu.Trigger>

            <DropdownMenu.Portal>
                <DropdownMenu.Content
                    className={cn(
                        "min-w-[200px] bg-[#1a1b23] border border-panel-outline rounded-md shadow-lg py-1 z-50",
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
                                    : "hover:bg-white/10 transition-colors cursor-pointer",
                                "outline-none",
                                "flex items-center justify-between",
                                "focus:bg-white/10"
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
