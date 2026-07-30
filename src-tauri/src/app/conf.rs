use log::{error, info};
use serde::Deserialize;
use serde_json::json;
use std::path::{Path, PathBuf};
use tauri::App;
use tauri_plugin_store::StoreBuilder;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub minutes_before: u32,
    pub mascot: String,
    pub calendar_ids: Vec<String>,
    pub sound_enabled: bool,
    pub launch_at_login: bool,
    pub ics_url_set: bool,
    pub mute_until: Option<i64>,
    /// násobič rychlosti přeletu (0.7 = rychleji, 1.4 = pomaleji)
    pub speed: f64,
    /// jazyk UI ("cs" | "en")
    pub language: String,
    /// onboarding při prvním spuštění už proběhl
    pub first_run_done: bool,
    /// co maskot říká: "title" | "fun" | "hybrid"
    pub text_mode: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            minutes_before: 2,
            mascot: "random".to_string(),
            calendar_ids: Vec::new(),
            sound_enabled: false,
            launch_at_login: false,
            ics_url_set: false,
            mute_until: None,
            speed: 1.0,
            language: "cs".to_string(),
            first_run_done: false,
            text_mode: "title".to_string(),
        }
    }
}

impl AppConfig {
    // Never panics: a missing/malformed settings.json falls back to
    // AppConfig::default() and logs the reason instead of crashing the app.
    // Zatím bez volajícího — použije ho scheduler (PR6) a settings/mute
    // commandy (PR7/PR8), proto ho necháváme místo mazání.
    #[allow(dead_code)]
    pub fn new() -> AppConfig {
        // Race se store zápisem: soubor může být na okamžik prázdný
        // (truncate→write). Jeden retry + cache poslední dobré konfigurace,
        // ať vadné čtení nikdy nevrátí čisté defaulty (např. zrušené mute).
        match Self::read_once() {
            Some(cfg) => {
                *LAST_GOOD.lock().unwrap() = Some(cfg.clone());
                cfg
            }
            None => {
                std::thread::sleep(std::time::Duration::from_millis(40));
                match Self::read_once() {
                    Some(cfg) => {
                        *LAST_GOOD.lock().unwrap() = Some(cfg.clone());
                        cfg
                    }
                    None => LAST_GOOD
                        .lock()
                        .unwrap()
                        .clone()
                        .unwrap_or_default(),
                }
            }
        }
    }

    fn read_once() -> Option<AppConfig> {
        let setting_path = combine_config_path("settings.json")?;
        let raw = std::fs::read_to_string(&setting_path).ok()?;
        let parsed: serde_json::Value = serde_json::from_str(&raw).ok()?;
        let app_value = parsed.get("app").cloned().unwrap_or_else(|| json!({}));
        // Per-field fallback: jedna špatně typovaná hodnota (ručně
        // upravený JSON) nesmí zahodit celé nastavení. Co se přečíst
        // nedá, spadne na default a zaloguje se.
        let d = AppConfig::default();
        let g = |k: &str| app_value.get(k).cloned();
        let cfg = AppConfig {
            minutes_before: g("minutesBefore")
                .and_then(|v| v.as_u64())
                .map(|v| v.min(60) as u32)
                .unwrap_or(d.minutes_before),
            mascot: g("mascot")
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or(d.mascot),
            calendar_ids: g("calendarIds")
                .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
                .unwrap_or(d.calendar_ids),
            sound_enabled: g("soundEnabled").and_then(|v| v.as_bool()).unwrap_or(d.sound_enabled),
            launch_at_login: g("launchAtLogin").and_then(|v| v.as_bool()).unwrap_or(d.launch_at_login),
            ics_url_set: g("icsUrlSet").and_then(|v| v.as_bool()).unwrap_or(d.ics_url_set),
            mute_until: g("muteUntil").and_then(|v| v.as_i64()),
            speed: g("speed")
                .and_then(|v| v.as_f64())
                .map(|v| v.clamp(0.4, 3.0))
                .unwrap_or(d.speed),
            language: g("language")
                .and_then(|v| v.as_str().map(str::to_string))
                .map(|v| if v == "en" { "en" } else { "cs" }.to_string())
                .unwrap_or(d.language),
            first_run_done: g("firstRunDone").and_then(|v| v.as_bool()).unwrap_or(d.first_run_done),
            text_mode: g("textMode")
                .and_then(|v| v.as_str().map(str::to_string))
                .filter(|v| v == "title" || v == "fun" || v == "hybrid")
                .unwrap_or(d.text_mode),
        };
        Some(cfg)
    }
}

static LAST_GOOD: std::sync::Mutex<Option<AppConfig>> = std::sync::Mutex::new(None);

pub fn convert_path(path_str: &str) -> Option<String> {
    if cfg!(target_os = "windows") {
        Some(path_str.replace('/', "\\"))
    } else {
        Some(path_str.replace('\\', "/"))
    }
}

pub fn app_root() -> PathBuf {
    match tauri::api::path::config_dir() {
        Some(dir) => dir.join("Ptacek"),
        None => {
            error!("Could not resolve OS config dir, falling back to current dir");
            PathBuf::from(".").join("Ptacek")
        }
    }
}

/// Vrací cestu ke konfiguraci. Whitelist: frontend smí dostat cestu
/// JEN k settings.json — jinak by kompromitované webview mohlo přes
/// store plugin zapisovat JSON kamkoli v domovském adresáři.
#[tauri::command(rename_all = "snake_case")]
pub fn combine_config_path(config_name: &str) -> Option<String> {
    if config_name != "settings.json" {
        error!("combine_config_path: odmítnuto neznámé jméno konfigurace");
        return None;
    }
    convert_path(app_root().join(config_name).to_str()?)
}

pub fn if_app_config_does_not_exist_create_default(app: &mut App, config_name: &str) {
    let setting_path = match combine_config_path(config_name) {
        Some(path) => path,
        None => {
            error!(
                "Could not resolve path for {}, skipping default creation",
                config_name
            );
            return;
        }
    };

    if Path::new(&setting_path).exists() {
        return;
    }

    let default_config = match config_name {
        "settings.json" => include_str!("default/settings.json"),
        _ => return,
    };

    let json_data: serde_json::Value = match serde_json::from_str(default_config) {
        Ok(value) => value,
        Err(err) => {
            error!("Bundled default {} is invalid JSON: {}", config_name, err);
            return;
        }
    };

    let mut store = StoreBuilder::new(app.handle(), PathBuf::from(setting_path)).build();

    if let Err(err) = store.insert("app".to_string(), json!(json_data)) {
        error!("Error inserting default config into store: {}", err);
        return;
    }

    if let Err(err) = store.save() {
        error!("Error saving default config {}: {}", config_name, err);
        return;
    }

    info!("Created default config file: {}", config_name);
}

/// Zapíše libovolný klíč do "app" objektu v settings store — stejné
/// místo, kam píše frontend (settings.ts).
pub fn set_app_value(app: &tauri::AppHandle, key: &str, value: serde_json::Value) {
    use tauri::Manager;
    use tauri_plugin_store::{with_store, StoreCollection};

    let path = match combine_config_path("settings.json") {
        Some(p) => PathBuf::from(p),
        None => {
            error!("set_app_value: nelze určit cestu settings.json");
            return;
        }
    };
    let stores = app.state::<StoreCollection<tauri::Wry>>();
    let key_owned = key.to_string();
    let result = with_store(app.clone(), stores, path, move |store| {
        let mut cfg = store.get("app").cloned().unwrap_or_else(|| json!({}));
        cfg[&key_owned] = value.clone();
        store.insert("app".to_string(), cfg)?;
        store.save()
    });
    if let Err(err) = result {
        error!("Zápis {key} selhal: {err}");
    }
}

/// Zapíše muteUntil (unix timestamp, None = zrušit) do settings store
/// pod klíč "app" — stejné místo, kam píše frontend (settings.ts).
pub fn set_mute_until(app: &tauri::AppHandle, until: Option<i64>) {
    use tauri::Manager;
    use tauri_plugin_store::{with_store, StoreCollection};

    let path = match combine_config_path("settings.json") {
        Some(p) => PathBuf::from(p),
        None => {
            error!("set_mute_until: nelze určit cestu settings.json");
            return;
        }
    };
    let stores = app.state::<StoreCollection<tauri::Wry>>();
    let result = with_store(app.clone(), stores, path, |store| {
        let mut cfg = store
            .get("app")
            .cloned()
            .unwrap_or_else(|| json!({}));
        cfg["muteUntil"] = match until {
            Some(ts) => json!(ts),
            None => serde_json::Value::Null,
        };
        store.insert("app".to_string(), cfg)?;
        store.save()
    });
    match result {
        Ok(()) => info!("muteUntil nastaven: {:?}", until),
        Err(err) => error!("Zápis muteUntil selhal: {err}"),
    }
}
