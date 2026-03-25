import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Dialog, DialogContent } from "./ui/dialog";
import { Button } from "./ui/button";
import { Download, RefreshCw, X } from "lucide-react";

export function UpdateDialog({ open, onOpenChange, updateInfo, onUpdateComplete }) {
  const [isInstalling, setIsInstalling] = useState(false);
  const [error, setError] = useState("");

  async function handleInstall() {
    if (isInstalling) return;
    setIsInstalling(true);
    setError("");

    try {
      await invoke("install_app_update");
      // On Windows, the app will automatically close and installation will proceed after install_app_update completes
      // On macOS/Linux, the app will automatically restart after installation
      // This function will not execute code below on success as the app will exit
    } catch (e) {
      setError(e?.message ?? String(e));
      setIsInstalling(false);
    }
  }

  function handleSkip() {
    onOpenChange?.(false);
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        onEscapeKeyDown={(e) => {
          if (!isInstalling) e.preventDefault();
        }}
        onPointerDownOutside={(e) => {
          if (!isInstalling) e.preventDefault();
        }}
      >
        <div className="flex flex-col gap-4">
          <div>
            <div className="text-lg font-semibold">Update Available</div>
            <div className="mt-1 text-sm text-white/55">
              A new version is available. Would you like to update now?
            </div>
          </div>

          {updateInfo && (
            <div className="rounded-2xl border border-panel-outline bg-black/20 px-4 py-3">
              <div className="grid gap-2 text-sm">
                <div className="flex items-center justify-between">
                  <span className="text-white/60">Current Version:</span>
                  <span className="font-mono">{updateInfo.current_version}</span>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-white/60">New Version:</span>
                  <span className="font-mono font-semibold text-green-400">
                    {updateInfo.version}
                  </span>
                </div>
                {updateInfo.date && (
                  <div className="flex items-center justify-between">
                    <span className="text-white/60">Release Date:</span>
                    <span className="text-white/70">
                      {new Date(updateInfo.date).toLocaleDateString("en-US")}
                    </span>
                  </div>
                )}
              </div>
            </div>
          )}

          {updateInfo?.body && (
            <div className="rounded-2xl border border-panel-outline bg-black/20 px-4 py-3">
              <div className="mb-2 text-xs font-semibold text-white/70">
                Release Notes
              </div>
              <div className="max-h-40 overflow-auto text-xs text-white/60 whitespace-pre-wrap">
                {updateInfo.body}
              </div>
            </div>
          )}

          {error && (
            <div className="rounded-2xl border border-red-400/20 bg-red-400/10 px-3 py-2 text-xs text-red-200">
              {error}
            </div>
          )}

          <div className="flex items-center justify-end gap-2">
            {!isInstalling && (
              <Button
                variant="outline"
                className="h-10"
                onClick={handleSkip}
              >
                <X className="h-4 w-4" />
                Later
              </Button>
            )}
            <Button
              variant="default"
              className="h-10"
              disabled={isInstalling}
              onClick={handleInstall}
            >
              {isInstalling ? (
                <>
                  <RefreshCw className="h-4 w-4 animate-spin" />
                  Installing...
                </>
              ) : (
                <>
                  <Download className="h-4 w-4"/>
                  Install Update
                </>
              )}
            </Button>
          </div>

          {isInstalling && (
            <div className="text-[11px] text-white/40">
              The app will automatically restart after the update is complete.
            </div>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
