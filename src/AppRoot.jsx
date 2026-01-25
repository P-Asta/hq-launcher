import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import LauncherPage from "./pages/LauncherPage";
import { LoginDialog } from "./components/auth/LoginDialog";
import { UpdateDialog } from "./components/UpdateDialog";
import Titlebar from "./Titlebar";

function Splash({ message }) {
  return (
    <div className="flex h-full w-full items-center justify-center p-6 text-white">
      <div className="w-[min(520px,92vw)] rounded-3xl border border-white/10 bg-white/5 p-6">
        <div className="text-lg font-semibold">HQ Launcher</div>
        <div className="mt-2 text-sm text-white/55">{message}</div>
      </div>
    </div>
  );
}

export default function AppRoot() {
  const [loginState, setLoginState] = useState({
    status: "loading", // loading | ready
    is_logged_in: false,
    username: null,
  });
  const [bootstrapError, setBootstrapError] = useState("");
  const [loginOpen, setLoginOpen] = useState(false);
  const loginResolveRef = useRef(null);
  const [updateInfo, setUpdateInfo] = useState(null);
  const [updateDialogOpen, setUpdateDialogOpen] = useState(false);
  const updateCheckedRef = useRef(false);

  async function refreshLoginState() {
    try {
      const s = await invoke("depot_get_login_state");
      setLoginState({
        status: "ready",
        is_logged_in: !!s?.is_logged_in,
        username: s?.username ?? null,
      });
      setBootstrapError("");
    } catch (e) {
      setLoginState({
        status: "ready",
        is_logged_in: false,
        username: null,
      });
      setBootstrapError(e?.message ?? String(e));
    }
  }

  useEffect(() => {
    refreshLoginState();
  }, []);

  // Check for updates on app startup
  useEffect(() => {
    if (updateCheckedRef.current) return;
    updateCheckedRef.current = true;

    (async () => {
      try {
        const info = await invoke("check_app_update");
        if (info?.available) {
          setUpdateInfo(info);
          setUpdateDialogOpen(true);
        }
      } catch (e) {
        console.error("Failed to check for updates:", e);
        // Silently ignore update check failures to avoid disrupting user experience
      }
    })();
  }, []);

  function requestLogin() {
    setLoginOpen(true);
    return new Promise((resolve) => {
      loginResolveRef.current = resolve;
    });
  }

  async function logout() {
    try {
      await invoke("depot_logout");
    } catch {}
    refreshLoginState();
  }

  return (
    <div className="h-full w-full overflow-hidden">
      <Titlebar className="fixed top-0 left-0 h-10" />
      <div className="relative h-[calc(100vh-32px)] w-full mt-10">
        {loginState.status === "loading" ? (
          <Splash message="Starting up..." />
        ) : (
          <LauncherPage
            loginState={loginState}
            onLogout={logout}
            onRequireLogin={requestLogin}
            bootstrapError={bootstrapError}
          />
        )}

        <LoginDialog
          open={loginOpen}
          onLoggedIn={(s) => {
            setLoginState({
              status: "ready",
              is_logged_in: !!s?.is_logged_in,
              username: s?.username ?? null,
            });
            setLoginOpen(false);
            if (loginResolveRef.current) loginResolveRef.current(true);
            loginResolveRef.current = null;
          }}
        />

        <UpdateDialog
          open={updateDialogOpen}
          onOpenChange={setUpdateDialogOpen}
          updateInfo={updateInfo}
        />
      </div>
    </div>
  );
}
