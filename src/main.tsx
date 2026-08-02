import React from "react";
import ReactDOM from "react-dom/client";
import OverlayApp from "./overlay/OverlayApp";
import SettingsApp from "./settings/SettingsApp";
import WelcomeApp from "./welcome/WelcomeApp";
import "./styles.css";

// prevent right click context menu (matches the WindowPet fork behaviour)
document.addEventListener("contextmenu", (e) => e.preventDefault());

// Ptáček má tři okna, každé se otevírá na vlastní URL cestě (viz
// utils.rs open_setting_window, window.rs open_welcome_window/open_overlay).
// react-router-dom byl v PR1 vyhozen jako nepotřebná závislost.
const isSettingsWindow = window.location.pathname === "/setting";
const isWelcomeWindow = window.location.pathname === "/welcome";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {isSettingsWindow ? (
      <SettingsApp />
    ) : isWelcomeWindow ? (
      <WelcomeApp />
    ) : (
      <OverlayApp />
    )}
  </React.StrictMode>
);
