import React from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import App from "./AppRoot";
import "./index.css";
import {
  applyThemeSettings,
  loadStoredThemeBrightness,
  loadStoredThemeHue,
  loadStoredThemeMode,
} from "./lib/theme";
import { getWindowMode } from "./lib/windowMode";

applyThemeSettings({
  hue: loadStoredThemeHue(),
  brightness: loadStoredThemeBrightness(),
  mode: loadStoredThemeMode(),
});

const windowMode = getWindowMode();
if (windowMode === "game-overlay") {
  document.documentElement.classList.add("overlay-window");
  invoke("report_game_overlay_frontend_info", { message: "main.jsx detected game-overlay mode" }).catch(console.error);
}

ReactDOM.createRoot(document.getElementById("root")).render(<App />);
