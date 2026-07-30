use super::utils::open_setting_window;
use super::window;
use log::{error, info};
use tauri::{
    AppHandle, CustomMenuItem, Manager, SystemTray, SystemTrayEvent, SystemTrayMenu,
    SystemTrayMenuItem,
};

struct TrayTexts {
    try_now: &'static str,
    mute_1h: &'static str,
    mute_today: &'static str,
    muted_until: &'static str, // formát: "{} HH:MM …"
    snooze: &'static str,
    close_flyby: &'static str,
    setting: &'static str,
    quit: &'static str,
    about: &'static str,
}

fn texts(lang: &str) -> TrayTexts {
    if lang == "en" {
        TrayTexts {
            try_now: "Try now",
            mute_1h: "Mute for 1 hour",
            mute_today: "Mute until tomorrow",
            muted_until: "Muted until {} — click to unmute",
            snooze: "Snooze flyby by 5 minutes",
            close_flyby: "Close flyby",
            setting: "Settings…",
            quit: "Quit",
            about: "Made by Transformuj.ai",
        }
    } else {
        TrayTexts {
            try_now: "Vyzkoušet teď",
            mute_1h: "Ztlumit na 1 hodinu",
            mute_today: "Ztlumit do konce dne",
            muted_until: "Ztlumeno do {} — kliknutím zrušíš",
            snooze: "Odložit o 5 minut",
            close_flyby: "Zavřít přelet",
            setting: "Nastavení…",
            quit: "Ukončit",
            about: "Vyrobeno v Transformuj.ai",
        }
    }
}

pub fn init_system_tray() -> SystemTray {
    let t = texts(&super::conf::AppConfig::new().language);
    let menu = SystemTrayMenu::new()
        .add_item(CustomMenuItem::new("try_now".to_string(), t.try_now))
        .add_item(CustomMenuItem::new("mute_1h".to_string(), t.mute_1h))
        .add_item(CustomMenuItem::new("mute_today".to_string(), t.mute_today))
        .add_native_item(SystemTrayMenuItem::Separator)
        // P1.5: klávesnicová/tray cesta k „Odložit"/„Zavřít" — ekvivalent
        // hover karty na overlayi, který je záměrně click-through a bez
        // fokusu. Enabled jen dokud fakt nějaký přelet letí (viz
        // window::open_overlay / close_overlay).
        .add_item(CustomMenuItem::new("tray_snooze".to_string(), t.snooze).disabled())
        .add_item(CustomMenuItem::new("tray_close".to_string(), t.close_flyby).disabled())
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new("setting".to_string(), t.setting))
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new("quit".to_string(), t.quit))
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new("about".to_string(), t.about));

    SystemTray::new().with_menu(menu)
}

/// Přepíše titulky tray položek podle jazyka (volá se po přepnutí v UI).
pub fn apply_language(app: &AppHandle) {
    let cfg = super::conf::AppConfig::new();
    let t = texts(&cfg.language);
    let tray = app.tray_handle();
    let _ = tray.get_item("try_now").set_title(t.try_now);
    let _ = tray.get_item("tray_snooze").set_title(t.snooze);
    let _ = tray.get_item("tray_close").set_title(t.close_flyby);
    let _ = tray.get_item("setting").set_title(t.setting);
    let _ = tray.get_item("quit").set_title(t.quit);
    let _ = tray.get_item("about").set_title(t.about);
    let now = chrono::Local::now().timestamp();
    set_mute_titles(app, cfg.mute_until.filter(|ts| *ts > now));
}

/// Zapne/vypne „Odložit o 5 minut" a „Zavřít přelet" v tray menu — platí
/// jen, když nějaký přelet fakt letí (window::open_overlay/close_overlay).
pub fn set_flyby_actions_enabled(app: &AppHandle, enabled: bool) {
    let tray = app.tray_handle();
    let _ = tray.get_item("tray_snooze").set_enabled(enabled);
    let _ = tray.get_item("tray_close").set_enabled(enabled);
}

/// Promítne stav ztlumení do titulků tray položek, ať je na první pohled
/// vidět, že ztlumení platí (a že jde kliknutím zrušit).
fn set_mute_titles(app: &AppHandle, until: Option<i64>) {
    let t = texts(&super::conf::AppConfig::new().language);
    let tray = app.tray_handle();
    match until {
        Some(ts) => {
            let time = chrono::DateTime::from_timestamp(ts, 0)
                .map(|dt| dt.with_timezone(&chrono::Local).format("%H:%M").to_string())
                .unwrap_or_default();
            let label = t.muted_until.replace("{}", &time);
            // Stav patří JEN na tu položku, kterou uživatel použil —
            // jinak menu vypadá, že je tam dvakrát totéž. Druhá zůstává
            // funkční, takže ztlumení jde prodloužit do konce dne.
            if is_end_of_day(ts) {
                let _ = tray.get_item("mute_today").set_title(&label);
                let _ = tray.get_item("mute_1h").set_title(t.mute_1h);
            } else {
                let _ = tray.get_item("mute_1h").set_title(&label);
                let _ = tray.get_item("mute_today").set_title(t.mute_today);
            }
        }
        None => {
            let _ = tray.get_item("mute_1h").set_title(t.mute_1h);
            let _ = tray.get_item("mute_today").set_title(t.mute_today);
        }
    }
}

/// Je čas přesně půlnoc? (tak vypadá „ztlumit do konce dne")
fn is_end_of_day(ts: i64) -> bool {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt| {
            let local = dt.with_timezone(&chrono::Local);
            local.format("%H:%M").to_string() == "00:00"
        })
        .unwrap_or(false)
}

pub fn handle_tray_event(app: &AppHandle, event: SystemTrayEvent) {
    if let SystemTrayEvent::MenuItemClick { id, .. } = event {
        match id.as_str() {
            // Zatím jen log — reálný přelet/mute logika přibude ve
            // scheduleru (PR6), tohle jsou jen zaháknuté položky menu.
            "try_now" => {
                info!("Tray: Vyzkoušet teď — spouštím demo přelet");
                window::open_overlay(app, "mode=demo&mascot=random");
            }
            // P1.5: stejná akce jako hover karta na overlayi, jen bez
            // myši. Enabled jen když window::active_overlay() něco vrátí
            // (tray_snooze/tray_close jsou jinak disabled — viz
            // set_flyby_actions_enabled), takže „nic neletí" tu prakticky
            // nenastane; kdyby ano, tiše nic neuděláme.
            "tray_snooze" => match window::active_overlay() {
                Some(info) => {
                    info!("Tray: Odložit o 5 minut");
                    super::cmd::snooze_flyby(app.clone(), info.title, info.time, info.mascot);
                }
                None => info!("Tray: Odložit o 5 minut — žádný přelet zrovna neletí"),
            },
            "tray_close" => {
                info!("Tray: Zavřít přelet");
                window::close_overlay(app);
            }
            "mute_1h" => {
                let cfg = super::conf::AppConfig::new();
                let now = chrono::Local::now().timestamp();
                let active_hour_mute = cfg
                    .mute_until
                    .is_some_and(|t| t > now && !is_end_of_day(t));
                if active_hour_mute {
                    super::conf::set_mute_until(app, None);
                    set_mute_titles(app, None);
                } else {
                    let until = now + 3600;
                    super::conf::set_mute_until(app, Some(until));
                    set_mute_titles(app, Some(until));
                }
            }
            "mute_today" => {
                let cfg = super::conf::AppConfig::new();
                let now = chrono::Local::now().timestamp();
                let active_day_mute = cfg
                    .mute_until
                    .is_some_and(|t| t > now && is_end_of_day(t));
                if active_day_mute {
                    super::conf::set_mute_until(app, None);
                    set_mute_titles(app, None);
                } else {
                    // do půlnoci místního času
                    let until = chrono::Local::now()
                        .date_naive()
                        .succ_opt()
                        .and_then(|d| d.and_hms_opt(0, 0, 0))
                        .and_then(|dt| dt.and_local_timezone(chrono::Local).single())
                        .map(|dt| dt.timestamp())
                        .unwrap_or(now + 24 * 3600);
                    super::conf::set_mute_until(app, Some(until));
                    set_mute_titles(app, Some(until));
                }
            }
            "setting" => match app.get_window("setting") {
                Some(window) => {
                    if let Err(err) = window.set_focus() {
                        error!("Failed to focus setting window: {}", err);
                    }
                    info!("Setting window already open, focused it");
                }
                None => {
                    open_setting_window(app.clone());
                }
            },
            "quit" => {
                info!("Ukončuji Ptáčka");
                app.exit(0);
            }
            // Pevná URL bez uživatelského vstupu — jediné místo, kde appka
            // otevírá web, a jde o Rust stranu (webview síť nemá).
            "about" => {
                if let Err(err) = std::process::Command::new("/usr/bin/open")
                    .arg("https://transformuj.ai")
                    .spawn()
                {
                    error!("Otevření transformuj.ai selhalo: {err}");
                }
            }
            _ => {}
        }
    }
}
