import React from "react";
import ReactDOM from "react-dom/client";
import App from "./AppRoot";
import "./index.css";
import {
  applyThemeSettings,
  loadStoredThemeBrightness,
  loadStoredThemeHue,
} from "./lib/theme";

applyThemeSettings({
  hue: loadStoredThemeHue(),
  brightness: loadStoredThemeBrightness(),
});

ReactDOM.createRoot(document.getElementById("root")).render(<App />);
