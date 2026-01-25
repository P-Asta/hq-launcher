import { getCurrentWindow } from '@tauri-apps/api/window';
import { useEffect, useState } from 'react';
import { Minus, Square, X } from 'lucide-react';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import { cn } from './lib/cn';
import { invoke } from '@tauri-apps/api/core';
import { emit, listen } from '@tauri-apps/api/event';
import { register } from '@tauri-apps/plugin-global-shortcut';
import { Dialog, DialogContent } from './components/ui/dialog';
import { Button } from './components/ui/button';


export default function Titlebar({ ...props }) {
    const [isMaximized, setIsMaximized] = useState(false);
    const [configLinkState, setConfigLinkState] = useState(null);
    const [configLinkBusy, setConfigLinkBusy] = useState(false);
    const [linkWarnOpen, setLinkWarnOpen] = useState(false);
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
    const handleClose = () => {
        console.log('close');
        appWindow.close()};

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



    return (
        <>
            <Dialog open={linkWarnOpen} onOpenChange={setLinkWarnOpen}>
                <DialogContent
                    onEscapeKeyDown={(e) => {
                        if (!configLinkBusy) e.preventDefault();
                    }}
                    onPointerDownOutside={(e) => {
                        if (!configLinkBusy) e.preventDefault();
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

            <div 
                data-tauri-drag-region 
                className={cn('w-full flex items-center justify-between px-2 border-b border-white/10 bg-[#0b0c10]/80 backdrop-blur-sm z-50', props.className)}
            >
                {/* Left side - Menu items */}
                <div className="flex items-center gap-1">
                    <img src="/icon.svg" alt="logo" className='ml-2 w-6 h-6' />
                    <TitlebarMenu 
                        name="File" 
                        items={[
                            // { label: "Open App Settings", shortcut: "CommandOrControl+,", action: () => console.log('Open settings') },
                            { label: "Open Version Folder", shortcut: "CommandOrControl+O", action: () => {invoke('open_version_folder')} }
                        ]} 
                    />
                    <TitlebarMenu 
                        name="Edit" 
                        items={[
                            { label: "Unlink Config", shortcut: "", disabled: unlinkDisabled, action: unlinkConfig },
                            { label: "Link Config", shortcut: "", disabled: linkDisabled, action: linkConfig }
                        ]} 
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
    let registeredShortcuts = [];


    useEffect(() => {
        items.forEach(item => {
            if (item.shortcut) {
                if (registeredShortcuts.includes(item.shortcut)) { return }
                register(item.shortcut, () => {
                    item.action?.();
                    console.log('shortcut registered', item.shortcut);
                })
            }
        });
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
                        "min-w-[200px] bg-[#1a1b23] border border-white/10 rounded-md shadow-lg py-1 z-50",
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
                                <span className="text-xs text-white/50 ml-4">{invoke('get_global_shortcut', { shortcut: item.shortcut })}</span>
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
