import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { appWindow } from "@tauri-apps/api/window";
import { getStrings, Lang } from "../i18n";
import { getSettings } from "../utils/settings";
import lockup from "../assets/transformuj-lockup.png";
import "./welcome.css";

// WELCOME: první okno, které uživatel po instalaci uvidí. Rust ho
// vytváří skryté (stejný vzor jako overlay/OverlayApp) a zavírá se přes
// command welcome_done, který zároveň spustí uvítací demo přelet.
//
// Animace „vsáknutí" je čistě CSS (transform + opacity) — po jejím konci
// (transitionend, s fallback timeoutem) frontend zavolá welcome_done.

const VANISH_MS = 800; // fallback, kdyby transitionend nikdy nedorazil

function prefersReducedMotion(): boolean {
  try {
    return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  } catch {
    return false;
  }
}

function WelcomeApp() {
  const shown = useRef(false);
  const doneRef = useRef(false);
  const cardRef = useRef<HTMLDivElement>(null);
  const [lang, setLang] = useState<Lang>("cs");
  const [vanishing, setVanishing] = useState(false);
  const reduced = useMemo(prefersReducedMotion, []);

  useEffect(() => {
    if (shown.current) return; // StrictMode double-mount guard
    shown.current = true;
    getSettings()
      .then((s) => setLang(s.language === "en" ? "en" : "cs"))
      .catch(() => undefined)
      .finally(() => {
        appWindow.show().catch(() => undefined);
      });
  }, []);

  const finish = () => {
    if (doneRef.current) return;
    doneRef.current = true;
    invoke("welcome_done").catch(() => undefined);
  };

  const handleOk = () => {
    if (reduced) {
      finish();
      return;
    }
    setVanishing(true);
    setTimeout(finish, VANISH_MS);
  };

  const t = getStrings(lang);

  return (
    <div className="welcome-root">
      <div
        ref={cardRef}
        className={vanishing ? "welcome-card vanish" : "welcome-card"}
        onTransitionEnd={(e) => {
          if (e.propertyName === "opacity") finish();
        }}
      >
        <img className="welcome-logo" src={lockup} alt="Transformuj.ai" />

        <h1 className="welcome-title">{t.welcomeTitle}</h1>
        <p className="welcome-thanks">{t.welcomeThanks}</p>

        <p className="welcome-beta">{t.welcomeBeta}</p>

        <div className="welcome-where">
          <div className="menubar-mock" aria-hidden="true">
            <div className="menubar-icon" />
            <div className="menubar-icon" />
            <div className="menubar-icon menubar-icon-bird">🐦</div>
            <div className="menubar-icon" />
          </div>
          <p className="welcome-where-text">{t.welcomeWhere}</p>
          <p className="welcome-where-hint">{t.welcomeWhereHint}</p>
        </div>

        <button className="welcome-ok" onClick={handleOk}>
          {t.welcomeOk}
        </button>
      </div>
    </div>
  );
}

export default WelcomeApp;
