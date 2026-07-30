// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
use app::{cmd, conf, tray};
use log::{error, info};
use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_log::LogTarget;

#[derive(Clone, serde::Serialize)]
struct Payload {
    args: Vec<String>,
    cwd: String,
}

fn build_app() {
    match tauri::Builder::default()
        // Autostart je defaultně OFF (viz AppConfig::launch_at_login) a bez
        // placeholder argumentů — spike předával nesmyslné --flag1 --flag2.
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(
            tauri_plugin_log::Builder::default()
                .targets([
                    LogTarget::Folder(app::conf::app_root()),
                    LogTarget::Stdout,
                ])
                // strop a rotace — log nesmí růst donekonečna
                .max_file_size(1_000_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            info!("Single instance triggered: {argv:?}, cwd: {cwd}");

            if let Err(err) = app.emit_all("single-instance", Payload { args: argv, cwd }) {
                error!("Failed to emit single-instance event: {}", err);
            }
        }))
        .setup(move |app| {
            // Nejčastější chyba při instalaci: uživatel appku spustí
            // rovnou z připojeného DMG. Pak se chová divně (nastavení
            // i oprávnění zmizí, jakmile DMG odpojí). Poradíme mu.
            // Spuštění z DMG nebo z „App Translocation" (macOS appku
            // stažených z internetu spustí z náhodné cesty jen pro čtení,
            // pokud se do Aplikací nedostane přetažením ve Finderu).
            // V obou případech se nastavení ani oprávnění neuloží.
            #[cfg(target_os = "macos")]
            if std::env::current_exe()
                .map(|p| {
                    let s = p.to_string_lossy().to_string();
                    s.starts_with("/Volumes/") || s.contains("/AppTranslocation/")
                })
                .unwrap_or(false)
            {
                let _ = std::process::Command::new("/usr/bin/osascript")
                    .arg("-e")
                    .arg(
                        "display alert \"Ptáčka je potřeba nejdřív nainstalovat\" \
                         message \"Teď běží z dočasného umístění, takže si nezapamatuje \
                         nastavení ani povolení kalendáře.\n\nOtevři obraz disku Ptáček, \
                         přetáhni ikonu myší do složky Aplikace a spusť ho odtamtud. \
                         Tuhle kopii pak můžeš zavřít.\" \
                         as critical buttons {\"Rozumím\"} default button 1",
                    )
                    .spawn();
            }

            // Accessory: appka běží jako menu bar utilita, žádná ikona
            // v Docku a žádný app switcher záznam.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Žádné okno se nevytváří při startu (tauri.conf.json windows: []).
            // Overlay okno se bude vytvářet dynamicky ve window.rs (PR2).
            conf::if_app_config_does_not_exist_create_default(app, "settings.json");

            // Srdce appky: plánovač přeletů z kalendáře.
            app::scheduler::start(app.handle());

            // Watcher na opakované spuštění: druhá instance nechá marker
            // a skončí; my na něj do vteřiny otevřeme Nastavení.
            {
                let handle = app.handle();
                let marker = app::conf::app_root().join("open-settings-request");
                let _ = std::fs::remove_file(&marker);
                tauri::async_runtime::spawn(async move {
                    loop {
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        if marker.exists() {
                            let _ = std::fs::remove_file(&marker);
                            app::utils::open_setting_window(handle.clone());
                        }
                    }
                });
            }

            // První spuštění: otevřít Nastavení (jinak uživatel neví, že
            // appka vůbec běží — sedí jen v liště) a po chvíli pustit
            // uvítací přelet ptáčka. Flag se zapíše hned, ať se onboarding
            // neopakuje.
            if !app::conf::AppConfig::new().first_run_done {
                let handle = app.handle();
                app::conf::set_app_value(&handle, "firstRunDone", serde_json::json!(true));
                app::utils::open_setting_window(handle.clone());
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
                    app::window::open_overlay(&handle, "mode=demo&mascot=bird");
                });
            }

            // Testovací hák: PTACEK_DEMO=1 spustí demo přelet 2 s po startu.
            // Tray menu nejde spolehlivě proklikat automatizovaně (overflow
            // v přeplněném menu baru), tohle dává QA deterministickou cestu.
            #[cfg(debug_assertions)]
            if let Ok(demo) = std::env::var("PTACEK_DEMO") {
                // PTACEK_DEMO=1 → náhodný maskot; PTACEK_DEMO=<id> → konkrétní.
                let mut query = if demo == "1" {
                    "mode=demo&mascot=random".to_string()
                } else {
                    format!("mode=demo&mascot={demo}")
                };
                // Test-only: PTACEK_DEMO_TITLE musí být už URL-encoded
                // (produkční payload skládá scheduler s řádným encodingem).
                if let Ok(title) = std::env::var("PTACEK_DEMO_TITLE") {
                    query.push_str(&format!("&title={title}"));
                }
                let handle = app.handle();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    app::window::open_overlay(&handle, &query);
                });
            }

            // QA hák: PTACEK_EVENTS=1 → status oprávnění, případný TCC
            // dialog a výpis událostí na příštích 24 h do logu.
            // (env NEBO marker soubor — `open` přes launchd env nepředá
            // a spouštění ze sandboxovaného shellu blokuje kalendářová data)
            #[cfg(debug_assertions)]
            {
            let qa_marker = app::conf::app_root().join("qa-events");
            if std::env::var("PTACEK_EVENTS").is_ok() || qa_marker.exists() {
                // marker je jednorázový — jinak by appka logovala názvy
                // schůzek při každém startu
                let _ = std::fs::remove_file(&qa_marker);
                tauri::async_runtime::spawn(async {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    let status = app::calendar::eventkit::authorization_status();
                    info!("EventKit status: {status}");
                    let granted = status == "authorized"
                        || app::calendar::eventkit::request_access(
                            std::time::Duration::from_secs(120),
                        );
                    if granted {
                        let cals = app::calendar::eventkit::list_calendars();
                        info!("EventKit: {} kalendářů", cals.len());
                        for c in cals.iter().take(8) {
                            info!("  kalendář: {}", c.title);
                        }
                        let evs = app::calendar::eventkit::fetch_events(168.0, &[]);
                        info!("EventKit: {} událostí v příštích 7 dnech", evs.len());
                        for e in evs.iter().take(5) {
                            info!("  event: {} @ {}", e.title, e.start);
                        }
                    } else {
                        info!("EventKit: přístup neudělen ({status})");
                    }
                });
            }
            }

            // QA hák: PTACEK_SETTINGS=1 otevře okno nastavení po startu
            // (tray klik nejde automatizovat v přeplněném menu baru).
            #[cfg(debug_assertions)]
            if std::env::var("PTACEK_SETTINGS").is_ok() {
                app::utils::open_setting_window(app.handle());
            }

            info!("Ptáček started");
            Ok(())
        })
        .system_tray(tray::init_system_tray())
        .on_system_tray_event(tray::handle_tray_event)
        .invoke_handler(tauri::generate_handler![
            conf::combine_config_path,
            cmd::get_mouse_position,
            cmd::overlay_done,
            cmd::set_overlay_interactive,
            cmd::trigger_demo,
            cmd::set_launch_at_login,
            cmd::open_transformuj,
            cmd::open_linkedin,
            cmd::open_calendar_privacy_settings,
            cmd::open_github_issues,
            cmd::set_ics_url,
            cmd::clear_ics_url,
            cmd::test_ics_url,
            cmd::calendar_status,
            cmd::request_calendar_access,
            cmd::list_calendars,
            cmd::upcoming_count,
            cmd::calendars_changed,
            cmd::snooze_flyby,
            cmd::refresh_tray,
            cmd::open_mail_info,
            cmd::open_mail_jakub,
            cmd::open_partner,
            cmd::uninstall_app,
        ])
        .build(tauri::generate_context!())
    {
        // Sestavení appky selhat prakticky nemůže; kdyby ano, ať po sobě
        // nechá důvod v logu místo holého panic hlášení.
        Ok(app) => app.run(|_app_handle, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                api.prevent_exit();
            }
        }),
        Err(err) => {
            error!("Ptáček se nepodařilo spustit: {err}");
            std::process::exit(1);
        }
    }
}

fn main() {
    // Panic hook: zaloguj panic (např. do log pluginu) místo tichého pádu
    // bez stopy. Panic samotný appku ukončí (release profil má panic="abort"),
    // ale log s příčinou přežije v ~/Library/Logs.
    std::panic::set_hook(Box::new(|panic_info| {
        error!("Panic: {}", panic_info);
    }));

    // Single-instance zámek: tauri-plugin-single-instance na macOS v1
    // nefunguje (duplicitní tray ikony), takže vlastní flock. Zámek drží
    // po celou dobu běhu; druhá instance ho nezíská a tiše skončí.
    let lock_path = app::conf::app_root().join("ptacek.lock");
    let _ = std::fs::create_dir_all(app::conf::app_root());
    let lock_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        // zámek jen držíme, obsah nezajímá — truncate by ho zbytečně mazal
        .truncate(false)
        .open(&lock_path);
    let _lock_guard = match lock_file {
        Ok(f) => {
            use fs2::FileExt;
            if f.try_lock_exclusive().is_err() {
                // Uživatel otevřel appku podruhé (z Aplikací/DMG) — bez
                // reakce by si myslel, že se nic nestalo. Necháme běžící
                // instanci otevřít Nastavení (watcher níže) a končíme.
                let _ = std::fs::write(
                    app::conf::app_root().join("open-settings-request"),
                    b"1",
                );
                return;
            }
            Some(f) // guard žije do konce main
        }
        Err(_) => None, // zámek se nepovedl → radši spustit než nespustit
    };

    build_app();
}
