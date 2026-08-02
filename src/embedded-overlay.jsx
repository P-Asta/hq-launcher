import React from "react";
import ReactDOM from "react-dom/client";
import GameOverlay from "./GameOverlay";
import "./index.css";
import {
  applyThemeSettings,
  loadStoredThemeBrightness,
  loadStoredThemeHue,
  loadStoredThemeMode,
} from "./lib/theme";

window.__HQ_EMBEDDED_OVERLAY__ = true;
document.documentElement.classList.add("overlay-window");
applyThemeSettings({
  hue: loadStoredThemeHue(),
  brightness: loadStoredThemeBrightness(),
  mode: loadStoredThemeMode(),
});

ReactDOM.createRoot(document.getElementById("root")).render(<GameOverlay />);
