use log::error;
use mouse_position::mouse_position::Mouse;
use serde_json::json;
use tauri::Manager;

use super::window;

/// Frontend hlásí konec přeletu (animationend) — zavři overlay okno.
#[tauri::command]
pub fn overlay_done(app: tauri::AppHandle) {
    window::close_overlay(&app);
}

/// Hover nad maskotem: true = okno přijímá kliky (detail karta),
/// false = zpět na click-through.
#[tauri::command]
pub fn set_overlay_interactive(app: tauri::AppHandle, interactive: bool) {
    if interactive {
        // hover = uživatel s oknem pracuje → failsafe ať ho nezavře
        window::touch_keepalive();
    }
    if let Some(w) = app.get_window(window::OVERLAY_LABEL) {
        if let Err(err) = w.set_ignore_cursor_events(!interactive) {
            error!("set_overlay_interactive({interactive}) selhalo: {err}");
        }
    }
}

#[tauri::command]
pub fn get_mouse_position() -> serde_json::Value {
    /*
     * because we set the window to ignore cursor events, we cannot use
     * javascript to get the mouse position, so we use get mouse position manually
     */
    let position = Mouse::get_mouse_position();
    match position {
        Mouse::Position { x, y } => {
            json!({
                "clientX": x,
                "clientY": y
            })
        }
        Mouse::Error => {
            error!("Error getting mouse position");
            json!(null)
        }
    }
}

/// Demo přelet z okna Nastavení. Vrací, jestli se přelet fakt spustil —
/// UI tak dokáže ukázat hlášku, když ne (typicky: přelet už běží).
#[tauri::command]
pub fn trigger_demo(app: tauri::AppHandle, mascot: Option<String>) -> bool {
    let id = mascot.unwrap_or_else(|| "random".to_string());
    // id jde z našeho UI (výběr z manifestu), přesto whitelist znaků —
    // payload nesmí umět rozbít query string.
    let safe: String = id.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    window::open_overlay(&app, &format!("mode=demo&mascot={safe}"))
}

/// Zapnutí/vypnutí spouštění po přihlášení (LaunchAgent přes plugin).
#[tauri::command]
pub fn set_launch_at_login(app: tauri::AppHandle, enable: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    let result = if enable { manager.enable() } else { manager.disable() };
    result.map_err(|e| {
        error!("Autostart změna selhala: {e}");
        e.to_string()
    })
}

/// Odložit přelet o 5 minut. Zavře overlay a naplánuje nový přelet se
/// stejným payloadem. Explicitní přání uživatele → vystřelí i po začátku
/// schůzky; respektuje jen ztlumení platné v čase odloženého přeletu.
#[tauri::command]
pub fn snooze_flyby(app: tauri::AppHandle, title: String, time: String, mascot: String) {
    // title přichází z webview → nedůvěryhodné: sanitizace + limit
    let title = super::calendar::sanitize_title(&title);
    let time: String = time.chars().take(16).collect();
    window::close_overlay(&app);
    let safe_mascot: String = mascot.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    // title přišel z našeho payloadu (už sanitizovaný Rustem), ale při
    // skládání query ho znovu percent-encodujeme.
    let query = format!(
        "mode=event&mascot={safe_mascot}&title={}&time={}",
        super::scheduler::percent_encode(&title),
        super::scheduler::percent_encode(&time),
    );
    log::info!("Snooze: přelet za 5 minut");
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(300)).await;
        let cfg = super::conf::AppConfig::new();
        let now = chrono::Local::now().timestamp();
        if cfg.mute_until.is_some_and(|t| t > now) {
            log::info!("Snooze zrušen — mezitím zapnuto ztlumení");
            return;
        }
        // Kolize s jiným přeletem → zkusit ještě dvakrát po 20 s,
        // ať se odložená schůzka tiše neztratí.
        for _ in 0..3 {
            if window::open_overlay(&app, &query) {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_secs(20)).await;
        }
        log::warn!("Snooze: přelet se nepodařilo zobrazit ani na třetí pokus");
    });
}

/// Stav oprávnění ke kalendáři pro settings UI.
#[tauri::command]
pub fn calendar_status() -> String {
    super::calendar::eventkit::authorization_status().to_string()
}

/// Vyžádá přístup ke kalendáři (TCC dialog). Volá se ze settings UI;
/// blokující část běží mimo main thread (async command = vlastní vlákno).
#[tauri::command(async)]
pub fn request_calendar_access() -> bool {
    let granted = super::calendar::eventkit::request_access(std::time::Duration::from_secs(120));
    if granted {
        // ať se schůzky načtou hned, ne až za 5 minut
        super::scheduler::request_poll();
    }
    granted
}

/// Kolik schůzek appka právě teď vidí (příštích 24 h, dle vybraných
/// kalendářů) — zpětná vazba pro nastavení.
#[tauri::command(async)]
pub fn upcoming_count(app: tauri::AppHandle) -> usize {
    let _ = app;
    let cfg = super::conf::AppConfig::new();
    super::calendar::eventkit::fetch_events(24.0, &cfg.calendar_ids).len()
}

/// Uživatel změnil výběr kalendářů → přeplánovat hned.
#[tauri::command]
pub fn calendars_changed() {
    super::scheduler::request_poll();
}

/// Seznam kalendářů pro checkboxy v nastavení.
#[tauri::command(async)]
pub fn list_calendars() -> Vec<super::calendar::eventkit::CalInfo> {
    super::calendar::eventkit::list_calendars()
}

/// Otevře web Transformuj — pevná URL.
#[tauri::command]
pub fn open_transformuj() {
    open_fixed("https://transformuj.ai");
}

/// Otevře v Nastavení systému rovnou panel Soukromí a zabezpečení →
/// Kalendáře. Pevný deep-link, žádný vstup z frontendu — stejný vzor
/// jako ostatní pevné otevírání URL v tomhle souboru. Používá se, když
/// je přístup „denied"/„restricted"/„writeOnly": systémový TCC dialog
/// se v těchto stavech znovu neukáže, takže je potřeba to udělat ručně.
#[tauri::command]
pub fn open_calendar_privacy_settings() {
    open_fixed("x-apple.systempreferences:com.apple.preference.security?Privacy_Calendars");
}

/// GitHub issues Ptáčka — pevná URL, veřejná betaverze prosí o hlášení chyb.
#[tauri::command]
pub fn open_github_issues() {
    open_fixed("https://github.com/transformuj-ai/ptacek/issues");
}

/// Otevře Jakubův LinkedIn — pevná URL.
#[tauri::command]
pub fn open_linkedin() {
    open_fixed("https://www.linkedin.com/in/liskajakub-cz/");
}

/// Po přepnutí jazyka v UI přepíše titulky tray menu (jazyk samotný
/// ukládá frontend do store).
#[tauri::command]
pub fn refresh_tray(app: tauri::AppHandle) {
    super::tray::apply_language(&app);
}

/// Uloží tajnou iCal URL do Keychainu (validace: https, doména, port).
/// Vrací chybovou hlášku pro UI, URL se nikde neloguje.
#[tauri::command(async)]
pub fn set_ics_url(url: String) -> Result<(), String> {
    super::calendar::ics::set_url(url.trim())
}

/// Odebere iCal URL z Keychainu.
#[tauri::command(async)]
pub fn clear_ics_url() {
    super::calendar::ics::clear_url();
}

/// Rychlý test iCal URL pro UI: stáhne feed a vrátí počet událostí
/// v příštích 7 dnech (bez detailů). Chyba stahování je chyba, ne nula —
/// UI ji ukáže jako problém, ne jako prázdný kalendář.
#[tauri::command(async)]
pub async fn test_ics_url() -> Result<usize, String> {
    super::calendar::ics::fetch_events(168.0)
        .await
        .map(|evs| evs.len())
}

/// Kanál partnera — pevná URL.
#[tauri::command]
pub fn open_partner() {
    open_fixed("https://www.youtube.com/@aisrozumem");
}

/// Napsat e-mail — pevné adresy.
#[tauri::command]
pub fn open_mail_info() {
    open_fixed("mailto:info@transformuj.ai");
}

#[tauri::command]
pub fn open_mail_jakub() {
    open_fixed("mailto:jakub@transformuj.ai");
}

/// Odinstalace: uklidí po sobě všechno, co appka na disku nechala, a
/// otevře Finder u sebe sama, ať uživateli zbude jediný krok — hodit
/// Ptáčka do koše. Přetažení do koše totiž samo nesmaže nastavení,
/// LaunchAgent pro spouštění po přihlášení ani adresu kalendáře
/// v Klíčence, a to je přesně ten nepořádek, který po sobě slušná appka
/// nenechává. Bundle nemažeme sami: běžící aplikace, která si pod rukama
/// smaže vlastní kód, je recept na tichou chybu.
#[tauri::command]
pub fn uninstall_app(app: tauri::AppHandle) {
    use tauri_plugin_autostart::ManagerExt;

    if let Err(err) = app.autolaunch().disable() {
        error!("Odinstalace: vypnutí autostartu selhalo: {err}");
    }
    super::calendar::ics::clear_url();

    // Mazat smíme JEN vlastní složku pod Application Support. Kontrola
    // je tu proto, aby ani podivně přenastavené prostředí nemohlo
    // proměnit odinstalaci v mazání něčeho cizího.
    let root = super::conf::app_root();
    if safe_to_remove(&root, tauri::api::path::config_dir().as_deref()) {
        if let Err(err) = std::fs::remove_dir_all(&root) {
            if err.kind() != std::io::ErrorKind::NotFound {
                error!("Odinstalace: smazání {} selhalo: {err}", root.display());
            }
        }
    } else {
        error!("Odinstalace: cesta {} nevypadá bezpečně, nemažu", root.display());
    }

    reveal_bundle_in_finder();

    // Rovnou ven, bez Tauri cleanupu — ten by store plugin donutil
    // settings.json zase založit a složka by se vrátila.
    log::info!("Odinstalace dokončena, končím");
    std::process::exit(0);
}

/// Smí odinstalace tuhle složku smazat? Jediná destruktivní operace
/// v celé aplikaci, proto explicitní pravidla místo důvěry v to, že
/// `app_root()` vrátí, co má: musí to být přímo naše složka „Ptacek"
/// uvnitř Application Support, ne ta složka samotná a ne nic výš.
fn safe_to_remove(root: &std::path::Path, base: Option<&std::path::Path>) -> bool {
    let Some(base) = base else { return false };
    root.is_absolute()
        && root.starts_with(base)
        && root != base
        && root.parent() == Some(base)
        && root.file_name().is_some_and(|name| name == "Ptacek")
}

/// Ukáže .app bundle ve Finderu (vybraný, připravený k přetažení do
/// koše). Když se cesta k bundlu nedá určit, otevře aspoň Aplikace.
fn reveal_bundle_in_finder() {
    let bundle = std::env::current_exe().ok().and_then(|exe| {
        exe.ancestors()
            .find(|p| p.extension().is_some_and(|e| e == "app"))
            .map(std::path::Path::to_path_buf)
    });
    let mut cmd = std::process::Command::new("/usr/bin/open");
    match bundle {
        Some(path) => {
            cmd.arg("-R").arg(path);
        }
        None => {
            cmd.arg("/Applications");
        }
    }
    if let Err(err) = cmd.spawn() {
        error!("Odinstalace: otevření Finderu selhalo: {err}");
    }
}

/// Jediná cesta appky k otevírání webu: konstantní URL, žádný vstup.
fn open_fixed(url: &'static str) {
    if let Err(err) = std::process::Command::new("/usr/bin/open").arg(url).spawn() {
        error!("Otevření {url} selhalo: {err}");
    }
}

#[cfg(test)]
mod uninstall_tests {
    use super::safe_to_remove;
    use std::path::Path;

    const BASE: &str = "/Users/nekdo/Library/Application Support";

    #[test]
    fn nase_slozka_se_smaze() {
        assert!(safe_to_remove(
            Path::new("/Users/nekdo/Library/Application Support/Ptacek"),
            Some(Path::new(BASE))
        ));
    }

    #[test]
    fn cizi_slozky_nikdy() {
        // sourozenci jiných aplikací ani nic mimo Application Support
        for cesta in [
            "/Users/nekdo/Library/Application Support/Slack",
            "/Users/nekdo/Library/Application Support",
            "/Users/nekdo/Library",
            "/Users/nekdo",
            "/Applications/Ptacek.app",
            "/",
        ] {
            assert!(
                !safe_to_remove(Path::new(cesta), Some(Path::new(BASE))),
                "{cesta} se nikdy mazat nesmí"
            );
        }
    }

    #[test]
    fn jen_primy_potomek() {
        // ani vnořená cesta, která jménem sedí
        assert!(!safe_to_remove(
            Path::new("/Users/nekdo/Library/Application Support/neco/Ptacek"),
            Some(Path::new(BASE))
        ));
    }

    #[test]
    fn bez_znameho_zakladu_nemazeme() {
        // config_dir() nedostupný → radši nedělat nic
        assert!(!safe_to_remove(
            Path::new("/Users/nekdo/Library/Application Support/Ptacek"),
            None
        ));
    }

    #[test]
    fn relativni_cesta_nikdy() {
        // fallback z app_root() při nedostupném config_dir je "./Ptacek"
        assert!(!safe_to_remove(Path::new("./Ptacek"), Some(Path::new("."))));
    }
}
