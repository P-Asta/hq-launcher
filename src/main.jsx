import React from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import "./index.css";
import {
  applyThemeSettings,
  loadStoredThemeBrightness,
  loadStoredThemeHue,
  loadStoredThemeMode,
} from "./lib/theme";
import { getWindowMode } from "./lib/windowMode";
import { HqLoading } from "./components/launcher/HqLoading";

applyThemeSettings({
  hue: loadStoredThemeHue(),
  brightness: loadStoredThemeBrightness(),
  mode: loadStoredThemeMode(),
});

const windowMode = getWindowMode();
const overlayWindow = !__HQ_ANDROID__ && ["game-overlay", "obs-overlay"].includes(windowMode);
if (overlayWindow) {
  document.documentElement.classList.add("overlay-window");
  if (windowMode === "game-overlay") invoke("report_game_overlay_frontend_info", { message: "main.jsx detected game-overlay mode" }).catch(console.error);
}

const App = React.lazy(() =>
  __HQ_ANDROID__ ? import("./pages/AndroidLauncher") : import("./AppRoot")
);

ReactDOM.createRoot(document.getElementById("root")).render(
  <React.Suspense fallback={overlayWindow ? null : <HqLoading screen />}>
    <App />
  </React.Suspense>
);
