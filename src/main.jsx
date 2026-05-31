import React from "react";
import ReactDOM from "react-dom/client";
import App from "./AppRoot";
import "./index.css";
import { applyPrimaryColor, loadStoredPrimaryColor } from "./lib/theme";

applyPrimaryColor(loadStoredPrimaryColor());

ReactDOM.createRoot(document.getElementById("root")).render(<App />);
