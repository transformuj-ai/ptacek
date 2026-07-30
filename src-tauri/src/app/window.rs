use log::{error, info};
use mouse_position::mouse_position::Mouse;
use tauri::{AppHandle, Manager, WindowBuilder, WindowUrl};

pub const OVERLAY_LABEL: &str = "overlay";

/// Poslední keep-alive z hoveru — failsafe podle něj pozná, že
/// uživatel s oknem pracuje (a nezavře mu ho pod rukama).
static LAST_KEEPALIVE: std::sync::OnceLock<std::sync::Mutex<std::time::Instant>> =
    std::sync::OnceLock::new();

fn keepalive_cell() -> &'static std::sync::Mutex<std::time::Instant> {
    LAST_KEEPALIVE.get_or_init(|| {
        std::sync::Mutex::new(
            std::time::Instant::now() - std::time::Duration::from_secs(3600),
        )
    })
}

/// Volá hover z frontendu — okno je „živé", failsafe počká.
pub fn touch_keepalive() {
    if let Ok(mut g) = keepalive_cell().lock() {
        *g = std::time::Instant::now();
    }
}

/// P1.5: payload právě letícího přeletu — tray potřebuje stejná data,
/// jaká má hover karta (title/time/mascot), aby „Odložit o 5 minut"
/// a „Zavřít přelet" fungovaly i bez myši nad maskotem.
#[derive(Clone, Default)]
pub struct OverlayInfo {
    pub title: String,
    pub time: String,
    pub mascot: String,
}

static ACTIVE_OVERLAY: std::sync::OnceLock<std::sync::Mutex<Option<OverlayInfo>>> =
    std::sync::OnceLock::new();

fn active_overlay_cell() -> &'static std::sync::Mutex<Option<OverlayInfo>> {
    ACTIVE_OVERLAY.get_or_init(|| std::sync::Mutex::new(None))
}

/// Info o právě letícím přeletu pro tray akce; `None` = žádný neletí.
pub fn active_overlay() -> Option<OverlayInfo> {
    active_overlay_cell().lock().ok().and_then(|g| g.clone())
}

fn set_active_overlay(info: Option<OverlayInfo>) {
    if let Ok(mut g) = active_overlay_cell().lock() {
        *g = info;
    }
}

/// Minimální percent-decode (inverze `scheduler::percent_encode`) —
/// dost na to, co appka sama do URL zakóduje.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn query_param(query: &str, key: &str) -> String {
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == key {
                return percent_decode(v);
            }
        }
    }
    String::new()
}

/// Otevře overlay okno přes celý primární monitor a spustí přelet.
/// `query` jde do URL (např. "mode=demo"); payload čte frontend přes
/// URLSearchParams. Okno existuje jen po dobu přeletu — zavírá ho
/// command `overlay_done` (animationend), nebo failsafe timer níže.
pub fn open_overlay(app: &AppHandle, query: &str) -> bool {
    if app.get_window(OVERLAY_LABEL).is_some() {
        info!("Overlay už existuje, nový přelet ignoruji");
        return false;
    }

    // Rychlost a jazyk z nastavení — jediné místo, kde se do payloadu
    // přidávají; frontend je jen přečte (CSS --speed, texty hover karty).
    let cfg = super::conf::AppConfig::new();
    let speed = cfg.speed.clamp(0.4, 3.0);
    let lang = if cfg.language == "en" { "en" } else { "cs" };
    let mode_text = match cfg.text_mode.as_str() {
        "fun" => "fun",
        "hybrid" => "hybrid",
        _ => "title",
    };
    let url = format!("/?{query}&speed={speed}&lang={lang}&text={mode_text}");

    // Zvuk platí pro KAŽDÝ přelet (ostrý i zkušební) — dřív hrál jen ze
    // scheduleru a u demo tlačítka působil rozbité.
    if cfg.sound_enabled {
        if let Ok(mut child) = std::process::Command::new("/usr/bin/afplay")
            .arg("/System/Library/Sounds/Pop.aiff")
            .spawn()
        {
            // sklidit potomka, ať nezůstávají zombie záznamy
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
    }
    let window = match WindowBuilder::new(app, OVERLAY_LABEL, WindowUrl::App(url.into()))
        .transparent(true)
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .focused(false)
        .visible(false)
        .build()
    {
        Ok(w) => w,
        Err(err) => {
            error!("Nepodařilo se vytvořit overlay okno: {err}");
            return false;
        }
    };

    // Roztáhnout přes celý cílový monitor. Monitor API je v Tauri v1
    // dostupné až na existujícím okně, proto se ptáme po buildu.
    match target_monitor(&window) {
        Some(monitor) => {
            // POZOR na pořadí: nejdřív velikost, pak pozice. AppKit při
            // resize drží spodní hranu okna, takže set_position→set_size
            // odsune horní okraj nad obrazovku (změřeno: Y = -552).
            if let Err(err) = window.set_size(*monitor.size()) {
                error!("Overlay set_size selhalo: {err}");
            }
            if let Err(err) = window.set_position(*monitor.position()) {
                error!("Overlay set_position selhalo: {err}");
            }
        }
        None => error!("Žádný monitor nenalezen, overlay zůstává v defaultní velikosti"),
    }

    // Click-through VŽDY jako výchozí stav — interaktivitu zapíná až hover
    // nad maskotem (command set_overlay_interactive). Pořadí je záměrné:
    // nejdřív ignore events, pak teprve zvednout window level (viz spike).
    if let Err(err) = window.set_ignore_cursor_events(true) {
        error!("Overlay set_ignore_cursor_events selhalo: {err}");
    }

    #[cfg(target_os = "macos")]
    raise_above_everything(&window);

    // Okno ukazuje až frontend po mountu (appWindow.show()) — bez bílého
    // flashe z cold-startu WebView. Failsafe: kdyby se frontend nikdy
    // neozval (zombie WebView), smyčka níže okno zavře při první
    // kontrole bez keep-alive (kontroluje se à 25 s).
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        // Failsafe: zombie okno zavřít. Když uživatel drží myš na
        // maskotovi (hover pauzuje animaci), okno žít smí — hover
        // posílá keep-alive a failsafe se posune.
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(25)).await;
            let Some(w) = app_handle.get_window(OVERLAY_LABEL) else {
                break;
            };
            let alive_recently = keepalive_cell()
                .lock()
                .map(|g| g.elapsed() < std::time::Duration::from_secs(25))
                .unwrap_or(false);
            if alive_recently {
                continue; // uživatel s ním pracuje
            }
            info!("Overlay failsafe: okno žije bez aktivity, zavírám");
            if let Err(err) = w.close() {
                error!("Overlay failsafe close selhalo: {err}");
            }
            break;
        }
    });

    // P1.5: uložit payload pro tray akce a zpřístupnit „Odložit"/„Zavřít"
    // v tray menu — ekvivalentní cesta pro klávesnici/VoiceOver, když se
    // uživatel nedostane myší nad maskota.
    let title = {
        let full = query_param(query, "full");
        if full.is_empty() {
            query_param(query, "title")
        } else {
            full
        }
    };
    set_active_overlay(Some(OverlayInfo {
        title,
        time: query_param(query, "time"),
        mascot: query_param(query, "mascot"),
    }));
    super::tray::set_flyby_actions_enabled(app, true);

    info!("Overlay okno vytvořeno ({query})");
    true
}

/// Na kterém monitoru se má přelet odehrát. Maskot má přeletět tam, kam
/// se uživatel dívá — tedy přes displej, na kterém má právě myš. Jeden
/// monitor = žádná otázka; když se kurzor nepodaří nikam zařadit
/// (souřadnice mimo všechny displeje po odpojení monitoru), padá to
/// zpátky na primární.
///
/// Souřadnice myši chodí v logických bodech (stejný prostor, ve kterém
/// pracuje hover hit-test), rámce monitorů jsou ve fyzických pixelech —
/// proto se každý monitor přepočítává vlastním scale faktorem.
fn target_monitor(window: &tauri::Window) -> Option<tauri::Monitor> {
    let monitors = window.available_monitors().ok()?;
    if monitors.len() <= 1 {
        return monitors.into_iter().next();
    }

    if let Mouse::Position { x, y } = Mouse::get_mouse_position() {
        let (mx, my) = (f64::from(x), f64::from(y));
        for monitor in &monitors {
            let scale = monitor.scale_factor();
            let pos = monitor.position().to_logical::<f64>(scale);
            let size = monitor.size().to_logical::<f64>(scale);
            if mx >= pos.x
                && mx < pos.x + size.width
                && my >= pos.y
                && my < pos.y + size.height
            {
                info!(
                    "Overlay poletí přes monitor {:?} (kurzor {mx}×{my})",
                    monitor.name()
                );
                return Some(monitor.clone());
            }
        }
        info!("Kurzor {mx}×{my} nepatří žádnému monitoru, beru primární");
    }

    window
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| monitors.into_iter().next())
}

pub fn close_overlay(app: &AppHandle) {
    set_active_overlay(None);
    super::tray::set_flyby_actions_enabled(app, false);
    if let Some(window) = app.get_window(OVERLAY_LABEL) {
        if let Err(err) = window.close() {
            error!("Zavření overlay okna selhalo: {err}");
        } else {
            info!("Overlay okno zavřeno");
        }
    }
}

/// Fix ze spiku: Tauri `alwaysOnTop` nechává okno na normální úrovni
/// (layer 0), takže ho aktivní aplikace překryjí a nad fullscreen Spaces
/// se nezobrazí vůbec. Řešení je nastavit NSWindow level a collection
/// behavior přímo přes Cocoa.
#[cfg(target_os = "macos")]
fn raise_above_everything(window: &tauri::Window) {
    // POZOR: NSWindow API (setLevel, setCollectionBehavior) smí běžet JEN
    // na hlavním vlákně — volání odjinud = SIGTRAP v AppKitu (ověřeno
    // crash reportem při spuštění z async tasku). run_on_main_thread
    // to řeší pro všechny volací cesty (tray, scheduler, demo hook).
    let w = window.clone();
    let result = window.run_on_main_thread(move || {
        use cocoa::base::id;
        use objc::{msg_send, sel, sel_impl};

        match w.ns_window() {
            Ok(ns_window) => {
                let ns_window = ns_window as id;
                unsafe {
                    // NSScreenSaverWindowLevel = 1000 — nad menu barem,
                    // Dockem i fullscreen aplikacemi. Fallback při
                    // problémech s dialogy: NSStatusWindowLevel = 25.
                    let _: () = msg_send![ns_window, setLevel: 1000_i64];

                    // canJoinAllSpaces (1<<0) | stationary (1<<4)
                    // | ignoresCycle (1<<6) | fullScreenAuxiliary (1<<8)
                    let behavior: u64 = (1 << 0) | (1 << 4) | (1 << 6) | (1 << 8);
                    let _: () = msg_send![ns_window, setCollectionBehavior: behavior];
                }
                info!("Overlay window level nastaven (screen-saver, all spaces)");
            }
            Err(err) => error!("ns_window nedostupné, level fix přeskočen: {err}"),
        }
    });

    if let Err(err) = result {
        error!("run_on_main_thread pro level fix selhalo: {err}");
    }
}

#[cfg(test)]
mod query_param_tests {
    use super::{percent_decode, query_param};

    #[test]
    fn dekoduje_procentove_escapy() {
        assert_eq!(percent_decode("Sch%C5%AFzka"), "Schůzka");
        assert_eq!(percent_decode("bez%20escapu"), "bez escapu");
        assert_eq!(percent_decode("nic"), "nic");
    }

    #[test]
    fn nedokoncene_escapy_na_konci_neshodi_dekoder() {
        assert_eq!(percent_decode("a%2"), "a%2");
        assert_eq!(percent_decode("a%"), "a%");
    }

    #[test]
    fn query_param_najde_hodnotu_podle_klice() {
        let q = "mode=event&mascot=bird&title=Sch%C5%AFzka&time=14%3A00";
        assert_eq!(query_param(q, "mascot"), "bird");
        assert_eq!(query_param(q, "title"), "Schůzka");
        assert_eq!(query_param(q, "time"), "14:00");
        assert_eq!(query_param(q, "full"), "");
    }
}
