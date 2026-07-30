import React from "react";
import ReactDOM from "react-dom/client";
import OverlayApp from "./overlay/OverlayApp";
import SettingsApp from "./settings/SettingsApp";
import "./styles.css";

// prevent right click context menu (matches the WindowPet fork behaviour)
document.addEventListener("contextmenu", (e) => e.preventDefault());

// Ptáček má jen dvě okna, každé se otevírá na vlastní URL cestě
// (viz utils.rs open_setting_window a budoucí window.rs pro overlay).
// react-router-dom byl v PR1 vyhozen jako nepotřebná závislost.
const isSettingsWindow = window.location.pathname === "/setting";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {isSettingsWindow ? <SettingsApp /> : <OverlayApp />}
  </React.StrictMode>
);
