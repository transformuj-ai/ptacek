import { useEffect, useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { appWindow } from "@tauri-apps/api/window";
import { getMascot } from "./mascots/manifest";
import { getStrings, Lang } from "../i18n";
import { useHover } from "./useHover";
import "./overlay.css";

// OVERLAY: přeletové okno Ptáčka. Okno vytváří Rust (window.rs) skryté,
// frontend ho ukáže až po mountu — bez bílého flashe. Konec master
// animace maskota hlásí onDone → overlay_done → Rust okno zavře.
// Failsafe 90 s drží Rust.
//
// Hover: pozici myši polluje Rust (okno je click-through); nad maskotem
// se scéna pauzne a ukáže karta s detailem + „Odložit o 5 min".
//
// Payload jde v URL query (URLSearchParams — hodnoty se nikdy
// neinterpolují do HTML): mascot, title, time, speed, mode.

// Bublina maskota letí přes obrazovku — musí se přečíst za pár vteřin.
// Delší názvy se zkracují; celý název je pak v kartě po najetí myší.
const MAX_BUBBLE = 78;
const MAX_BUBBLE_HYBRID = 52;

function readPayload() {
  const params = new URLSearchParams(window.location.search);
  const rawTitle = params.get("title") ?? "";
  const rawSpeed = Number(params.get("speed"));
  return {
    mode: params.get("mode") ?? "demo",
    mascotId: params.get("mascot"),
    // zkrácený název pro bublinu, plný pro kartu při hoveru
    title: rawTitle,
    full: params.get("full") ?? rawTitle,
    time: params.get("time") ?? "",
    lang: (params.get("lang") === "en" ? "en" : "cs") as Lang,
    textMode: params.get("text") ?? "title",
    speed:
      Number.isFinite(rawSpeed) && rawSpeed >= 0.4 && rawSpeed <= 3
        ? rawSpeed
        : 1,
  };
}

function OverlayApp() {
  const shown = useRef(false);
  const payload = useMemo(readPayload, []);
  const mascot = useMemo(() => getMascot(payload.mascotId), [payload.mascotId]);
  const hover = useHover();
  const cardPos = useRef<{ top: number; left: number } | null>(null);

  // Kartu umístit VEDLE zamrzlého maskota — pozici spočítat jednou při
  // vzniku hoveru, ať neskáče. NIKDY přes maskota: maskoti u spodní hrany
  // (kocour, robot, formule) dostanou kartu NAD sebe, letci pod sebe.
  if (hover && cardPos.current === null) {
    const CARD_W = 440;
    const CARD_H = 170;
    const GAP = 18;
    const act = document.querySelector<HTMLElement>(".act, .bd");
    if (act) {
      const r = act.getBoundingClientRect();
      const centerY = (r.top + r.bottom) / 2;
      let top: number;
      if (centerY > window.innerHeight * 0.55 && r.top - CARD_H - GAP >= 8) {
        // maskot dole → karta nad ním
        top = r.top - CARD_H - GAP;
      } else {
        // maskot nahoře/uprostřed → karta pod ním (s clampem k okraji)
        top = Math.min(r.bottom + GAP, window.innerHeight - CARD_H - 8);
      }
      const left = Math.min(
        Math.max(r.left, 16),
        window.innerWidth - CARD_W - 16
      );
      cardPos.current = { top, left };
    } else {
      cardPos.current = {
        top: window.innerHeight / 2,
        left: window.innerWidth / 2 - CARD_W / 2,
      };
    }
  }
  if (!hover && cardPos.current !== null) {
    cardPos.current = null;
  }

  useEffect(() => {
    if (shown.current) return; // StrictMode double-mount guard
    shown.current = true;
    appWindow.show().catch(() => {
      invoke("overlay_done").catch(() => undefined);
    });
  }, []);

  const handleDone = () => {
    invoke("overlay_done").catch(() => undefined);
  };

  const handleSnooze = () => {
    invoke("snooze_flyby", {
      title: payload.title || mascot.demoText,
      time: payload.time,
      mascot: mascot.id,
    }).catch(() => undefined);
  };

  const Mascot = mascot.component;
  const t = getStrings(payload.lang);

  // Co maskot říká — dle nastavení. Hlášky jsou obecné, takže nikdy
  // netvrdí nic konkrétního o schůzce (bezpečné i při sdílení obrazovky).
  const text = useMemo(() => {
    const full =
      payload.title || (payload.mode === "demo" ? mascot.demoText : "");
    const cut = (v: string, max: number) =>
      v.length > max ? `${v.slice(0, max).trimEnd()}…` : v;
    const fun = t.funLines[Math.floor(Math.random() * t.funLines.length)];
    if (payload.textMode === "fun") return fun;
    if (payload.textMode === "hybrid") {
      const short = cut(full, MAX_BUBBLE_HYBRID);
      return short ? `${fun} · ${short}` : fun;
    }
    return cut(full, MAX_BUBBLE);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div
      className={`overlay-root${hover ? " hovering" : ""}`}
      style={{ "--speed": payload.speed } as React.CSSProperties}
    >
      <div className={hover ? "scene-pause" : undefined}>
        <Mascot text={text} onDone={handleDone} />
      </div>
      {hover && (
        <div
          className="hover-card"
          style={
            cardPos.current
              ? { top: cardPos.current.top, left: cardPos.current.left, transform: "none" }
              : undefined
          }
        >
          <div className="hc-title">{payload.full || payload.title || text}</div>
          {payload.time && (
            <div className="hc-time">
              {payload.lang === "en" ? "Starts at" : "Začátek v"} {payload.time}
            </div>
          )}
          <div className="hc-actions">
            <button className="hc-snooze" onClick={handleSnooze}>
              {payload.lang === "en" ? "Snooze 5 min" : "Odložit o 5 min"}
            </button>
            <button className="hc-close" onClick={handleDone}>
              {payload.lang === "en" ? "Close" : "Zavřít"}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

export default OverlayApp;
