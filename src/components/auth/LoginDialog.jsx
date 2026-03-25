import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Dialog, DialogContent } from "../ui/dialog";
import { Button } from "../ui/button";
import { Input } from "../ui/input";

function isTwoFactorMessage(message) {
  const msg = String(message ?? "").toLowerCase();
  return (
    msg.includes("steam guard") ||
    msg.includes("two-factor") ||
    msg.includes("2fa") ||
    msg.includes("enter code") ||
    msg.includes("code required") ||
    msg.includes("code requested")
  );
}

export function LoginDialog({ open, onOpenChange, onLoggedIn }) {
  const onLoggedInRef = useRef(onLoggedIn);
  const waitingForSteamLoginResultRef = useRef(false);
  const steamLoginCodeTimerRef = useRef(null);
  const logContainerRef = useRef(null);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [twoFactorCode, setTwoFactorCode] = useState("");
  const [needs2fa, setNeeds2fa] = useState(false);
  const [busy, setBusy] = useState(false);
  const [loginRunning, setLoginRunning] = useState(false);
  const [error, setError] = useState("");
  const [hint, setHint] = useState("");
  const [logs, setLogs] = useState([]);
  const [sessionId, setSessionId] = useState(null);
  const [gameDownloadProgress, setGameDownloadProgress] = useState(0);

  useEffect(() => {
    onLoggedInRef.current = onLoggedIn;
  }, [onLoggedIn]);

  useEffect(() => {
    if (!open) return;
    const el = logContainerRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
  }, [logs, open]);

  function clearSteamLoginCodeTimer() {
    if (steamLoginCodeTimerRef.current != null) {
      clearTimeout(steamLoginCodeTimerRef.current);
      steamLoginCodeTimerRef.current = null;
    }
  }

  useEffect(() => {
    if (!open) return;

    setNeeds2fa(false);
    setBusy(false);
    setLoginRunning(false);
    setError("");
    setHint("");
    setLogs([]);
    setSessionId(null);
    setGameDownloadProgress(0);
    setTwoFactorCode("");
    waitingForSteamLoginResultRef.current = false;
    clearSteamLoginCodeTimer();

    let unlisten = null;
    (async () => {
      unlisten = await listen("depot-downloader", async (event) => {
        const p = event.payload;
        if (!p || typeof p !== "object") return;

        if (p.type === "NeedsTwoFactor") {
          if (p.data?.session_id != null) {
            setSessionId(Number(p.data.session_id));
          }
          setHint("");
          return;
        }

        if (p.type === "NeedsMobileConfirmation") {
          if (p.data?.session_id != null) {
            setSessionId(Number(p.data.session_id));
          }
          setNeeds2fa(false);
          setHint("");
          setError("Please complete the login approval in the Steam app.");
          return;
        }

        if (p.type === "LoginSuccess") {
          try {
            const state = await invoke("depot_get_login_state");
            onLoggedInRef.current?.(state);
          } catch {}
          setLoginRunning(false);
          setNeeds2fa(false);
          setHint("");
          setError("");
          waitingForSteamLoginResultRef.current = false;
          clearSteamLoginCodeTimer();
          return;
        }

        if (p.type === "Error" || p.type === "LoginFailed") {
          setLoginRunning(false);
          const msg = String(p.data ?? "");
          if (isTwoFactorMessage(msg)) {
            setNeeds2fa(true);
            setHint("Enter the Steam Guard code when it arrives.");
            setError("");
            return;
          }
          if (msg.includes("Login failed")) {
            setNeeds2fa(false);
          }
          setHint("");
          waitingForSteamLoginResultRef.current = false;
          clearSteamLoginCodeTimer();
          if (msg) {
            setError(msg);
          }
          return;
        }

        if (p.type === "Progress") {
          const current = Number(p.data?.current ?? 0);
          const total = Number(p.data?.total ?? 0);
          if (Number.isFinite(current) && Number.isFinite(total) && total > 0) {
            setGameDownloadProgress((current / total) * 100);
          }
          return;
        }

        if (p.type === "Output") {
          const line = String(p.data ?? "");
          if (!line) return;

          setLogs((prev) => [...prev, line].slice(-60));

          if (line.includes("Logging '") && line.includes("' into Steam3")) {
            waitingForSteamLoginResultRef.current = true;
            setNeeds2fa(false);
            setHint("");
            setError("");
            clearSteamLoginCodeTimer();
            steamLoginCodeTimerRef.current = setTimeout(() => {
              if (!waitingForSteamLoginResultRef.current) return;
              setNeeds2fa(true);
              setHint("");
              setError("");
            }, 3000);
          }
          if (
            waitingForSteamLoginResultRef.current &&
            line.trim() === "Done!"
          ) {
            waitingForSteamLoginResultRef.current = false;
            clearSteamLoginCodeTimer();
            setNeeds2fa(false);
            setHint("");
            setError("");
          }
          if (
            line.includes("STEAM GUARD") ||
            line.toLowerCase().includes("steam guard code requested") ||
            line.toLowerCase().includes("steam guard code required")
          ) {
            waitingForSteamLoginResultRef.current = false;
            clearSteamLoginCodeTimer();
            setNeeds2fa(true);
            setHint("Enter the Steam Guard code when it arrives.");
            setError("");
          }
          if (line.includes("Use the Steam Mobile App to confirm your sign in")) {
            waitingForSteamLoginResultRef.current = false;
            clearSteamLoginCodeTimer();
            setNeeds2fa(false);
            setSessionId(null);
            setHint("");
            setError("Use the Steam Mobile App to confirm your sign in.");
          }
          if (line.includes("1966721")) {
            waitingForSteamLoginResultRef.current = false;
            clearSteamLoginCodeTimer();
            setNeeds2fa(false);
            setHint("");
          }
        }
      });
    })();

    return () => {
      clearSteamLoginCodeTimer();
      if (typeof unlisten === "function") unlisten();
    };
  }, [open]);

  const isCodeEntryActive = loginRunning && needs2fa;

  const canSubmit = useMemo(() => {
    if (!loginRunning) {
      return !!username.trim() && !!password;
    }
    if (!needs2fa) return false;
    return !!twoFactorCode.trim() && sessionId != null;
  }, [loginRunning, needs2fa, password, sessionId, twoFactorCode, username]);

  async function submit() {
    if (loginRunning) return;
    if (!username.trim() || !password) return;

    setLoginRunning(true);
    setNeeds2fa(false);
    setError("");
    setHint("");
    setLogs([]);
    setGameDownloadProgress(0);
    setTwoFactorCode("");
    waitingForSteamLoginResultRef.current = false;
    clearSteamLoginCodeTimer();

    try {
      const sid = await invoke("depot_login_start", {
        username: username.trim(),
        password,
      });
      setSessionId(Number(sid));
    } catch (e) {
      setLoginRunning(false);
      setError(e?.message ?? String(e));
    }
  }

  async function submitCode() {
    const code = twoFactorCode.trim();
    if (!code || sessionId == null || busy) return;

    setBusy(true);
    setError("");
    setHint("");
    try {
      await invoke("depot_login_submit_code", { sessionId, code });
    } catch (e) {
      setError(e?.message ?? String(e));
    } finally {
      setBusy(false);
    }
  }

  const canClose = !loginRunning && !busy;

  function handleOpenChange(nextOpen) {
    if (!nextOpen && !canClose) return;
    onOpenChange?.(nextOpen);
  }

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent
        onEscapeKeyDown={(e) => {
          if (!canClose) e.preventDefault();
        }}
        onPointerDownOutside={(e) => {
          if (!canClose) e.preventDefault();
        }}
      >
        <div className="flex flex-col gap-4">
          <div>
            <div className="text-lg font-semibold">Steam Login</div>
            <div className="mt-1 text-sm text-white/55">
              You must log in before downloading Steam files with DepotDownloader.
            </div>
          </div>

          <div className="grid gap-3">
            <div>
              <div className="mb-1 text-xs font-semibold text-white/60">
                Username
              </div>
              <Input
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                placeholder="Steam ID"
                autoComplete="username"
              />
            </div>

            <div>
              <div className="mb-1 text-xs font-semibold text-white/60">
                Password
              </div>
              <Input
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="Password"
                type="password"
                autoComplete="current-password"
                onKeyDown={(e) => {
                  if (e.key === "Enter") submit();
                }}
              />
            </div>

            {needs2fa && (
              <div>
                <div className="mb-1 text-xs font-semibold text-white/60">
                  Steam Guard Code
                </div>
                <Input
                  value={twoFactorCode}
                  onChange={(e) => setTwoFactorCode(e.target.value)}
                  placeholder="12345"
                  inputMode="numeric"
                  onKeyDown={(e) => {
                    if (e.key === "Enter") submitCode();
                  }}
                />
              </div>
            )}
          </div>

          {logs.length > 0 && (
            <div className="rounded-2xl border border-panel-outline bg-black/20 px-3 py-2 text-[11px] text-white/60">
              <div className="mb-2 text-xs font-semibold text-white/70">
                DepotDownloader log
              </div>
              <div
                ref={logContainerRef}
                className="h-30 overflow-y-auto whitespace-pre-wrap pr-1"
              >
                {logs.map((l, i) => (
                  <div key={i}>{l}</div>
                ))}
              </div>
            </div>
          )}

          {gameDownloadProgress > 0 && (
            <div className="rounded-2xl border border-panel-outline bg-black/30 px-3 py-2 text-[11px] text-white/70">
              <div className="mb-1 flex items-center justify-between">
                <span className="font-semibold">Depot download progress</span>
                <span>{gameDownloadProgress.toFixed(1)}%</span>
              </div>
              <div className="h-1.5 w-full overflow-hidden rounded-full bg-white/10">
                <div
                  className="h-full rounded-full bg-emerald-400 transition-[width]"
                  style={{
                    width: `${Math.max(
                      0,
                      Math.min(100, gameDownloadProgress || 0)
                    )}%`,
                  }}
                />
              </div>
            </div>
          )}

          {error && (
            <div className="rounded-2xl border border-red-400/20 bg-red-400/10 px-3 py-2 text-xs text-red-200">
              {error}
            </div>
          )}

          {!error && isCodeEntryActive && (
            <div className="rounded-2xl border border-amber-300/25 bg-amber-400/10 px-3 py-2 text-xs text-amber-100">
              Steam Guard code required. Check your email or Steam app, then
              enter the code below and click Submit Code.
            </div>
          )}

          {!error && hint && (
            <div className="rounded-2xl border border-panel-outline bg-white/5 px-3 py-2 text-xs text-white/70">
              {hint}
            </div>
          )}

          <div className="flex items-center justify-end gap-2">
            <Button
              variant="outline"
              className="h-10"
              disabled={!canClose}
              onClick={() => handleOpenChange(false)}
            >
              Close
            </Button>
            <Button
              variant="default"
              className="h-10"
              disabled={!canSubmit || busy}
              onClick={isCodeEntryActive ? submitCode : submit}
            >
              {busy ? "Working..." : isCodeEntryActive ? "Submit Code" : "Login"}
            </Button>
          </div>

          <div className="text-[11px] text-white/40">
            Your login information is stored locally in the files created by
            DepotDownloader and the app's login state file.
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
