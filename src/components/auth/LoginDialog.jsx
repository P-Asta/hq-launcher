import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Dialog, DialogContent } from "../ui/dialog";
import { Button } from "../ui/button";
import { Input } from "../ui/input";

function isTwoFactorRequiredError(e) {
  const msg = e?.message ?? String(e ?? "");
  return msg.toLowerCase().includes("two-factor authentication required");
}

export function LoginDialog({ open, onLoggedIn }) {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [twoFactorCode, setTwoFactorCode] = useState("");
  const [needs2fa, setNeeds2fa] = useState(false);
  const [busy, setBusy] = useState(false); // only for short operations (submit code)
  const [loginRunning, setLoginRunning] = useState(false);
  const [error, setError] = useState("");
  const [logs, setLogs] = useState([]);
  const [sessionId, setSessionId] = useState(null);
  const [gameDownloadProgress, setGameDownloadProgress] = useState(0);

  useEffect(() => {
    if (!open) return;
    let unlisten = null;
    (async () => {
      unlisten = await listen("depot-downloader", (event) => {
        const p = event.payload;
        if (!p || typeof p !== "object") return;
        if (p.type === "NeedsTwoFactor") {
          // setNeeds2fa(true);
          // if (p.data?.session_id != null)
          //   setSessionId(Number(p.data.session_id));
          // if (p.data?.message) setError(String(p.data.message));
          // return;
        }
        if (p.type === "NeedsMobileConfirmation") {
          if (p.data?.session_id != null)
            setSessionId(Number(p.data.session_id));
          setError("Steam 앱에서 로그인 승인을 완료한 뒤 다시 시도해주세요.");
          return;
        }
        if (p.type === "LoginSuccess") {
          (async () => {
            const state = await invoke("depot_get_login_state");
            onLoggedIn?.(state);
          })().catch(() => {});
          setLoginRunning(false);
          return;
        }
        if (p.type === "Error") {
          setLoginRunning(false);
          const msg = String(p.data ?? "");
          if (msg.includes("Login failed")) {
            setNeeds2fa(false)
          }
          if (msg) setError(msg);
          return;
        }
        if (p.type === "LoginFailed") {
          setLoginRunning(false);
          const msg = String(p.data ?? "");
          if (msg) setError(msg);
          return;
        }
        if (p.type === "Progress") {
          const current = Number(p.data?.current ?? 0);
          const total = Number(p.data?.total ?? 0);
          if (Number.isFinite(current) && Number.isFinite(total) && total > 0) {
            const percent = (current / total) * 100;
            setGameDownloadProgress(percent);
          }
          return;
        }
        if (p.type === "Output") {
          const line = String(p.data ?? "");
          if (!line) return;
          setLogs((prev) => {
            const next = [...prev, line].slice(-60);
            if (line.includes("Connecting to Steam")) {
              setNeeds2fa(true);
              setSessionId(null);
            }
            if (line.includes("STEAM GUARD")) {
              setError(
                "When you receive the Steam Guard (email/app) code, enter it and click Submit code."
              );
            }
            if (line.includes("1966721")) {
              setNeeds2fa(false)
            }
            return next;
          });
        }
      });
    })();
    return () => {
      if (typeof unlisten === "function") unlisten();
    };
  }, [open]);

  const canSubmit = useMemo(() => {
    if (!loginRunning) {
      if (!username.trim()) return false;
      if (!password) return false;
      return true;
    }
    // while login is running, allow submitting code into the same process
    if (!twoFactorCode.trim()) return false;
    if (sessionId == null) return false;
    return true;
  }, [loginRunning, password, sessionId, twoFactorCode, username]);

  async function submit() {
    if (loginRunning) return;
    if (!username.trim() || !password) return;
    setLoginRunning(true);
    setError("");
    setLogs([]);
    try {
      // Start login and immediately ask user to check Steam Guard email/app.
      // setNeeds2fa(true);
      // setSessionId(null);
      // setError(
      //   "Steam Guard (email/app) 코드가 오면 입력 후 Submit code를 눌러주세요."
      // );

      // Start session and get sessionId immediately (no event race).
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
    try {
      await invoke("depot_login_submit_code", { sessionId, code });
      // keep waiting for the running depot_login() to finish
    } catch (e) {
      setError(e?.message ?? String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={() => {}}>
      <DialogContent
        onEscapeKeyDown={(e) => e.preventDefault()}
        onPointerDownOutside={(e) => e.preventDefault()}
      >
        <div className="flex flex-col gap-4">
          <div>
            <div className="text-lg font-semibold">Steam 로그인</div>
            <div className="mt-1 text-sm text-white/55">
              DepotDownloader로 Steam 파일을 받기 전에 로그인이 필요합니다.
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
                  Steam Guard (email/app)
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
            <div className="rounded-2xl border border-white/10 bg-black/20 px-3 py-2 text-[11px] text-white/60">
              <div className="mb-2 text-xs font-semibold text-white/70">
                DepotDownloader log
              </div>
              <div className="max-h-40 overflow-auto whitespace-pre-wrap">
                {logs.map((l, i) => (
                  <div key={i}>{l}</div>
                ))}
              </div>
            </div>
          )}

          {gameDownloadProgress > 0 && (
            <div className="rounded-2xl border border-white/10 bg-black/30 px-3 py-2 text-[11px] text-white/70">
              <div className="mb-1 flex items-center justify-between">
                <span className="font-semibold">게임 파일 다운로드 진행률</span>
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

          <div className="flex items-center justify-end gap-2">
            <Button
              variant="default"
              className="h-10"
              disabled={!canSubmit || busy}
              onClick={loginRunning ? submitCode : submit}
            >
              {busy ? "Working..." : loginRunning ? "Submit code" : "Login"}
            </Button>
          </div>

          <div className="text-[11px] text-white/40">
            로그인 정보는 DepotDownloader가 생성하는 파일과 앱의 로그인 상태
            파일로 로컬에 저장됩니다.
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
