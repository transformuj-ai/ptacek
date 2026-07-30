import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { appWindow } from "@tauri-apps/api/window";
import { getVersion } from "@tauri-apps/api/app";
import { MASCOTS } from "../overlay/mascots/manifest";
import { getStrings, Lang } from "../i18n";
import {
  AppSettings,
  DEFAULT_SETTINGS,
  getSettings,
  setSetting,
} from "../utils/settings";
import lockup from "../assets/transformuj-lockup.png";
import jakubPhoto from "../assets/jakub.png";
import filipPhoto from "../assets/filip.png";
import "./settings.css";

// Okno Nastavení — brand Transformuj (tmavá, oranžový akcent, mono labely).
// Každá změna se ukládá hned (žádné tlačítko Uložit). CZ/EN přes mini
// slovník v ../i18n.ts; tray se po přepnutí přepíše commandem refresh_tray.
//
// Verze se čte za běhu z bundlu (tauri.conf.json) — jediný zdroj pravdy,
// žádná konstanta, která by se při release zapomněla přepsat.

interface CalInfo {
  id: string;
  title: string;
  kind: string; // "birthday" | "subscription" | "normal"
}

function SettingsApp() {
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [loaded, setLoaded] = useState(false);
  const [calStatus, setCalStatus] = useState<string>("loading");
  const [calendars, setCalendars] = useState<CalInfo[]>([]);
  const [icsInput, setIcsInput] = useState("");
  const [icsBusy, setIcsBusy] = useState(false);
  const [icsMsg, setIcsMsg] = useState("");
  const [calMsg, setCalMsg] = useState("");
  const [calBusy, setCalBusy] = useState(false);
  const [tab, setTab] = useState<"settings" | "guide">("settings");
  const [version, setVersion] = useState("");
  const [confirmUninstall, setConfirmUninstall] = useState(false);
  // P1.6: fokus musí jít na potvrzovací tlačítko, když se objeví, a zpátky
  // na spouštěcí tlačítko po zrušení — jinak klávesnice/VoiceOver ztratí
  // pozici v UI.
  const uninstallTriggerRef = useRef<HTMLButtonElement>(null);
  const uninstallConfirmRef = useRef<HTMLButtonElement>(null);
  const [demoMsg, setDemoMsg] = useState("");
  const [autostartMsg, setAutostartMsg] = useState("");
  const [betaInfoOpen, setBetaInfoOpen] = useState(false);
  // Systémové „Omezit pohyb" — vysvětlíme, proč maskoti nelétají.
  const reducedMotion = useMemo(() => {
    try {
      return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    } catch {
      return false;
    }
  }, []);

  const lang: Lang = settings.language === "en" ? "en" : "cs";
  const t = getStrings(lang);

  async function loadCalendars() {
    try {
      const status = await invoke<string>("calendar_status");
      setCalStatus(status);
      if (status === "authorized") {
        setCalendars(await invoke<CalInfo[]>("list_calendars"));
      }
    } catch {
      setCalStatus("error");
    }
  }

  useEffect(() => {
    getVersion().then(setVersion).catch(() => undefined);
    getSettings()
      .then((s) => setSettings(s))
      .catch(() => undefined)
      .finally(() => setLoaded(true));
    loadCalendars();
  }, []);

  // Uživatel se často vrací z Nastavení systému (deep-link u zamítnutého
  // přístupu) — při návratu do okna Ptáčka rovnou přenačti stav, ať
  // nemusí ručně přepínat záložky, aby se stránka obnovila.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    appWindow
      .onFocusChanged(({ payload: focused }) => {
        if (focused) loadCalendars();
      })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => undefined);
    return () => unlisten?.();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function update<K extends keyof AppSettings>(
    key: K,
    value: AppSettings[K]
  ) {
    setSettings((prev) => ({ ...prev, [key]: value }));
    try {
      await setSetting(key, value);
    } catch {
      // uložení selhalo — další změna to zkusí znovu
    }
  }

  async function changeLanguage(value: string) {
    await update("language", value);
    invoke("refresh_tray").catch(() => undefined);
  }

  async function toggleAutostart(enable: boolean) {
    setAutostartMsg("");
    try {
      await invoke("set_launch_at_login", { enable });
      await update("launchAtLogin", enable);
    } catch {
      setSettings((prev) => ({ ...prev, launchAtLogin: !enable }));
      setAutostartMsg(t.autostartFail);
    }
  }

  async function grantCalendar() {
    setCalBusy(true);
    setCalMsg("");
    try {
      const granted = await invoke<boolean>("request_calendar_access");
      if (granted) {
        const cals = await invoke<CalInfo[]>("list_calendars");
        const events = await invoke<number>("upcoming_count");
        setCalendars(cals);
        setCalStatus("authorized");
        setCalMsg(events === 0 ? t.calOkNone : t.calOk(cals.length, events));
      } else {
        setCalMsg(t.calRefused);
      }
    } catch {
      setCalMsg(t.calRefused);
    } finally {
      setCalBusy(false);
      loadCalendars();
    }
  }

  // Prázdný seznam = „vše kromě narozenin a odebíraných svátků" —
  // celodenní položky z nich by jinak spouštěly přelet každý den.
  function isCalOn(id: string) {
    if (settings.calendarIds.length > 0) {
      return settings.calendarIds.includes(id);
    }
    const cal = calendars.find((c) => c.id === id);
    return cal ? cal.kind === "normal" : true;
  }

  function toggleCal(id: string, on: boolean) {
    const defaultOn = calendars.filter((c) => c.kind === "normal").map((c) => c.id);
    const current =
      settings.calendarIds.length === 0 ? defaultOn : settings.calendarIds;
    let next = on ? [...current, id] : current.filter((x) => x !== id);
    // shoda s výchozím stavem → uložit prázdný seznam (přežije i
    // kalendáře přidané v budoucnu)
    const sameAsDefault =
      next.length === defaultOn.length && defaultOn.every((x) => next.includes(x));
    if (sameAsDefault) next = [];
    update("calendarIds", next).then(() =>
      // přeplánovat hned, ne až za 5 minut
      invoke("calendars_changed").catch(() => undefined)
    );
  }

  async function saveIcs() {
    if (!icsInput.trim()) return;
    setIcsBusy(true);
    setIcsMsg("");
    try {
      await invoke("set_ics_url", { url: icsInput });
    } catch (e) {
      // validace adresy — česká hláška přímo z Rustu
      setIcsMsg(String(e));
      setIcsBusy(false);
      return;
    }
    try {
      // Stažení se testuje ZVLÁŠŤ od uložení: chyba stahování je chyba
      // („zkontroluj adresu"), ne „0 událostí" — to dřív mátlo.
      const count = await invoke<number>("test_ics_url");
      await update("icsUrlSet", true);
      setIcsInput("");
      setIcsMsg(t.icsOk(count));
    } catch (e) {
      await update("icsUrlSet", true); // adresa je uložená, jen teď nejde stáhnout
      setIcsInput("");
      setIcsMsg(t.icsFail(String(e)));
    } finally {
      setIcsBusy(false);
    }
  }

  function demo(mascotId?: string) {
    setDemoMsg("");
    invoke<boolean>("trigger_demo", { mascot: mascotId ?? settings.mascot })
      .then((started) => {
        if (!started) setDemoMsg(t.demoFail);
      })
      .catch(() => setDemoMsg(t.demoFail));
  }

  useEffect(() => {
    if (confirmUninstall) {
      uninstallConfirmRef.current?.focus();
    }
  }, [confirmUninstall]);

  if (!loaded) {
    return <div className="settings-root" />;
  }

  return (
    <div className="settings-root">
      <header className="s-head">
        <button
          type="button"
          className="s-head-logo img-button"
          onClick={() => invoke("open_transformuj").catch(() => undefined)}
          aria-label={t.linkWeb}
        >
          <img className="clickable" src={lockup} alt="Transformuj.ai" title="transformuj.ai" />
        </button>
        <div className="s-head-text">
          <div className="s-head-title-row">
            <h1>Ptáček</h1>
            <span className="beta-chip">{t.betaBadge}</span>
            <button
              type="button"
              className="beta-info-btn"
              aria-expanded={betaInfoOpen}
              aria-controls="beta-info-panel"
              onClick={() => setBetaInfoOpen((v) => !v)}
            >
              i<span className="visually-hidden">{t.betaInfoLabel}</span>
            </button>
          </div>
          <div className="s-sub">{t.tagline}</div>
          <div className="s-perk">{t.perk}</div>
        </div>
        <button className="s-demo" onClick={() => demo()}>
          {t.tryFlyby}
        </button>
      </header>

      {betaInfoOpen && (
        <div id="beta-info-panel" className="beta-info-panel" role="note">
          <p>{t.betaInfoText}</p>
          <button
            className="s-link"
            onClick={() => invoke("open_github_issues").catch(() => undefined)}
          >
            {t.betaReportBug}
          </button>
        </div>
      )}

      <div aria-live="polite">
        {demoMsg && <div className="s-info">{demoMsg}</div>}
        {autostartMsg && <div className="s-info">{autostartMsg}</div>}
      </div>

      <nav className="s-tabs" role="tablist">
        <button
          id="tab-settings"
          role="tab"
          aria-selected={tab === "settings"}
          aria-controls="panel-settings"
          className={tab === "settings" ? "s-tab active" : "s-tab"}
          onClick={() => setTab("settings")}
        >
          {t.tabSettings}
        </button>
        <button
          id="tab-guide"
          role="tab"
          aria-selected={tab === "guide"}
          aria-controls="panel-guide"
          className={tab === "guide" ? "s-tab active" : "s-tab"}
          onClick={() => setTab("guide")}
        >
          {t.tabGuide}
        </button>
      </nav>

      {tab === "guide" ? (
        <div role="tabpanel" id="panel-guide" aria-labelledby="tab-guide">
          <section>
            <div className="s-label">{t.tabGuide}</div>
            <div className="cal-hint">{t.guideIntro}</div>
          </section>
          <section>
            <div className="s-label">{t.guideStartTitle}</div>
            <ol className="ics-steps">
              {t.guideStart.map((x, i) => (
                <li key={i}>{x}</li>
              ))}
            </ol>
          </section>
          <section>
            <div className="s-label">{t.guideDailyTitle}</div>
            <ul className="ics-steps">
              {t.guideDaily.map((x, i) => (
                <li key={i}>{x}</li>
              ))}
            </ul>
          </section>
          <section>
            <div className="s-label">{t.secIcs}</div>
            <div className="cal-hint">{t.icsHint}</div>
            <ol className="ics-steps">
              {t.icsSteps.map((x, i) => (
                <li key={i}>{x}</li>
              ))}
            </ol>
            <div className="s-info">{t.icsSafety}</div>
          </section>
          <section>
            <div className="s-label">{t.guideTroubleTitle}</div>
            <ul className="ics-steps">
              {t.guideTrouble.map((x, i) => (
                <li key={i}>{x}</li>
              ))}
            </ul>
          </section>
          <section>
            <div className="s-label">{t.secSigning}</div>
            <div className="cal-hint">{t.signingText}</div>
          </section>
        </div>
      ) : (
        <div role="tabpanel" id="panel-settings" aria-labelledby="tab-settings">

      <section>
        <div className="s-label">{t.secFlyby}</div>
        <div className="s-row">
          <label htmlFor="set-minutes-before">{t.notifyBefore}</label>
          <select
            id="set-minutes-before"
            value={settings.minutesBefore}
            onChange={(e) => update("minutesBefore", Number(e.target.value))}
          >
            <option value={0}>{t.atStart}</option>
            {[1, 2, 5, 10].map((n) => (
              <option key={n} value={n}>
                {t.minBefore(n)}
              </option>
            ))}
          </select>
        </div>
        <div className="s-row">
          <label htmlFor="set-mascot">{t.mascot}</label>
          <select
            id="set-mascot"
            value={settings.mascot}
            onChange={(e) => update("mascot", e.target.value)}
          >
            <option value="random">{t.randomMascot}</option>
            {MASCOTS.map((m) => (
              <option key={m.id} value={m.id}>
                {lang === "en" ? m.nameEn : m.nazev}
              </option>
            ))}
          </select>
        </div>
        <div className="s-row">
          <label htmlFor="set-text-mode">{t.textMode}</label>
          <select
            id="set-text-mode"
            value={settings.textMode}
            onChange={(e) => update("textMode", e.target.value)}
          >
            <option value="title">{t.textModeTitle}</option>
            <option value="fun">{t.textModeFun}</option>
            <option value="hybrid">{t.textModeHybrid}</option>
          </select>
        </div>
        {settings.textMode !== "title" && (
          <div className="cal-hint">{t.textModeHint}</div>
        )}
        <div className="s-row">
          <label htmlFor="set-speed">{t.speed}</label>
          <select
            id="set-speed"
            value={String(settings.speed)}
            onChange={(e) => update("speed", Number(e.target.value))}
          >
            <option value="1.5">{t.speedSlow}</option>
            <option value="1">{t.speedNormal}</option>
            <option value="0.65">{t.speedFast}</option>
          </select>
        </div>
      </section>

      <section>
        <div className="s-label">{t.secBehavior}</div>
        <div className="s-row">
          <label htmlFor="set-launch-at-login">{t.launchAtLogin}</label>
          <input
            id="set-launch-at-login"
            type="checkbox"
            checked={settings.launchAtLogin}
            onChange={(e) => toggleAutostart(e.target.checked)}
          />
        </div>
        <div className="s-row">
          <label htmlFor="set-sound">{t.sound}</label>
          <input
            id="set-sound"
            type="checkbox"
            checked={settings.soundEnabled}
            onChange={(e) => update("soundEnabled", e.target.checked)}
          />
        </div>
        <div className="s-row">
          <label htmlFor="set-language">{t.language}</label>
          <select id="set-language" value={lang} onChange={(e) => changeLanguage(e.target.value)}>
            <option value="cs">Čeština</option>
            <option value="en">English</option>
          </select>
        </div>
        {reducedMotion && <div className="cal-hint">{t.motionNotice}</div>}
      </section>

      <section>
        <div className="s-label">{t.secMascots}</div>
        <div className="cal-hint">{t.mascotsHint}</div>
        <div className="mascot-list">
          {MASCOTS.map((m) => (
            <details key={m.id} className="mascot-item">
              <summary>{lang === "en" ? m.nameEn : m.nazev}</summary>
              <div className="mascot-body">
                <p>{lang === "en" ? m.descEn : m.descCs}</p>
                <button className="s-link" onClick={() => demo(m.id)}>
                  ▶ {t.play}
                </button>
              </div>
            </details>
          ))}
        </div>
      </section>

      <section>
        <div className="s-label">{t.secCalendars}</div>
        {calStatus === "authorized" && calendars.length > 0 && (
          <>
            <div className="cal-hint">{t.calHint}</div>
            <div className="cal-list">
              {calendars.map((c) => (
                <label key={c.id} className="cal-row">
                  <input
                    type="checkbox"
                    checked={isCalOn(c.id)}
                    onChange={(e) => toggleCal(c.id, e.target.checked)}
                  />
                  {c.title}
                  {c.kind !== "normal" && (
                    <em className="cal-tag">
                      {c.kind === "birthday"
                        ? lang === "en" ? "birthdays" : "narozeniny"
                        : lang === "en" ? "subscribed" : "odebíraný"}
                    </em>
                  )}
                </label>
              ))}
            </div>
          </>
        )}
        {calStatus === "authorized" && calendars.length === 0 && (
          <div className="s-info">{t.calEmpty}</div>
        )}
        {(calStatus === "notDetermined" || calStatus === "error") && (
          <div className="s-info">
            <div style={{ marginBottom: 10 }}>{t.calNoAccess}</div>
            <button className="s-link" onClick={grantCalendar} disabled={calBusy}>
              {calBusy ? t.calAsking : t.calGrant}
            </button>
          </div>
        )}
        <div aria-live="polite">
          {calMsg && <div className="s-perk">{calMsg}</div>}
        </div>
        {(calStatus === "denied" || calStatus === "restricted") && (
          <div className="s-info">
            <div style={{ marginBottom: 10 }}>{t.calDenied}</div>
            <button
              className="s-link"
              onClick={() =>
                invoke("open_calendar_privacy_settings").catch(() => undefined)
              }
            >
              {t.calOpenSystemSettings}
            </button>
          </div>
        )}
        {calStatus === "writeOnly" && (
          <div className="s-info">
            <div style={{ marginBottom: 10 }}>{t.calWriteOnly}</div>
            <button
              className="s-link"
              onClick={() =>
                invoke("open_calendar_privacy_settings").catch(() => undefined)
              }
            >
              {t.calOpenSystemSettings}
            </button>
          </div>
        )}
      </section>

      <section>
        <div className="s-label">{t.secIcs}</div>
        {!settings.icsUrlSet ? (
          <>
            <div className="cal-hint">{t.icsHint}</div>
            <ol className="ics-steps">
              {t.icsSteps.map((step, i) => (
                <li key={i}>{step}</li>
              ))}
            </ol>
            <div className="ics-row">
              <label htmlFor="set-ics-url" className="visually-hidden">
                {t.secIcs}
              </label>
              <input
                id="set-ics-url"
                type="password"
                placeholder="https://calendar.google.com/…/basic.ics"
                value={icsInput}
                onChange={(e) => setIcsInput(e.target.value)}
              />
              <button className="s-demo" onClick={saveIcs} disabled={icsBusy}>
                {icsBusy ? t.icsChecking : t.icsSave}
              </button>
            </div>
            <div aria-live="polite">
              {icsMsg && <div className="cal-hint">{icsMsg}</div>}
            </div>
            <div className="s-info">{t.icsSafety}</div>
          </>
        ) : (
          <div className="s-row">
            <span aria-live="polite">
              {t.icsSet} {icsMsg && <em className="cal-hint">({icsMsg})</em>}
            </span>
            <button
              className="s-link"
              onClick={async () => {
                await invoke("clear_ics_url").catch(() => undefined);
                await update("icsUrlSet", false);
                setIcsMsg("");
              }}
            >
              {t.icsRemove}
            </button>
          </div>
        )}
      </section>

      <section>
        <div className="s-label">{t.secSigning}</div>
        <div className="cal-hint">{t.signingText}</div>
      </section>

      <section>
        <div className="s-label">{t.secAbout}</div>
        <div className="s-about s-centered">
          <div className="brand-row">
            <div className="avatar-wrap">
              <button
                type="button"
                className="img-button"
                onClick={() =>
                  invoke("open_transformuj").catch(() => undefined)
                }
                aria-label={t.linkWeb}
              >
                <img
                  className="s-lockup clickable"
                  src={lockup}
                  alt="Transformuj.ai"
                  title="transformuj.ai"
                />
              </button>
              <div className="avatar-caption">Celý AI tým</div>
            </div>
            <div className="avatar-wrap">
              <button
                type="button"
                className="img-button"
                onClick={() => invoke("open_linkedin").catch(() => undefined)}
                aria-label={t.linkLi}
              >
                <img
                  className="avatar clickable"
                  src={jakubPhoto}
                  alt={t.photoAlt}
                  title="LinkedIn"
                />
              </button>
              <div className="avatar-caption">{t.photoCaption}</div>
            </div>
          </div>
          <p>
            <b>{t.whyTitle}</b> {t.whyText}
          </p>
          <p className="s-perk">{t.perk}</p>
          <p className="s-fine">
            {t.betaAboutLine}{" "}
            <button
              className="s-link s-link-inline"
              onClick={() => invoke("open_github_issues").catch(() => undefined)}
            >
              {t.betaReportBug}
            </button>
          </p>
          <p className="s-fine">{t.disclaimer(version)}</p>
          <div className="s-links">
            <button
              className="s-link s-link-primary"
              onClick={() => invoke("open_transformuj").catch(() => undefined)}
            >
              {t.linkWeb}
            </button>
            <button
              className="s-link"
              onClick={() => invoke("open_linkedin").catch(() => undefined)}
            >
              {t.linkLi}
            </button>
            <button
              className="s-link"
              onClick={() => invoke("open_mail_info").catch(() => undefined)}
            >
              info@transformuj.ai
            </button>
            <button
              className="s-link"
              onClick={() => invoke("open_mail_jakub").catch(() => undefined)}
            >
              jakub@transformuj.ai
            </button>
          </div>
        </div>
      </section>

      <section className="partner">
        <div className="s-label">{t.secPartner}</div>
        <div className="partner-head">
          <button
            type="button"
            className="img-button"
            onClick={() => invoke("open_partner").catch(() => undefined)}
            aria-label={t.partnerLink}
          >
            <img
              className="partner-avatar clickable"
              src={filipPhoto}
              alt={t.partnerName}
              title="YouTube: AI s rozumem"
            />
          </button>
          <div>
            <div className="partner-name">{t.partnerName}</div>
            <div className="partner-role">{t.partnerRole}</div>
            <div className="partner-quote">{t.partnerQuote}</div>
          </div>
        </div>
        <p className="partner-text">{t.partnerText}</p>
        <div className="partner-tags">
          {t.partnerTags.map((tag) => (
            <span key={tag}>{tag}</span>
          ))}
        </div>
        <button
          className="s-link"
          onClick={() => invoke("open_partner").catch(() => undefined)}
        >
          {t.partnerLink}
        </button>
      </section>

      <section className="uninstall">
        <div className="s-label">{t.secUninstall}</div>
        <p className="cal-hint">{t.uninstallText}</p>
        {confirmUninstall ? (
          <>
            <p className="uninstall-confirm">{t.uninstallConfirm}</p>
            <div className="uninstall-actions">
              <button
                ref={uninstallConfirmRef}
                className="btn-danger"
                onClick={() => invoke("uninstall_app").catch(() => undefined)}
              >
                {t.uninstallYes}
              </button>
              <button
                className="s-link"
                onClick={() => {
                  setConfirmUninstall(false);
                  uninstallTriggerRef.current?.focus();
                }}
              >
                {t.uninstallCancel}
              </button>
            </div>
          </>
        ) : (
          <button
            ref={uninstallTriggerRef}
            className="btn-danger-ghost"
            onClick={() => setConfirmUninstall(true)}
          >
            {t.uninstallBtn}
          </button>
        )}
      </section>
        </div>
      )}
    </div>
  );
}

export default SettingsApp;
