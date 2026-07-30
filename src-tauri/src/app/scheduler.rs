//! Scheduler přeletů: poll kalendáře → in-memory fronta → tick.
//! Runtime stav žije jen tady (Rust); frontend je hloupý renderer.
//!
//! Pravidla (dle plánu):
//! - poll à 5 min (+ hned po startu), tick à 15 s
//! - přelet v čase `start - minutes_before`, max 1× na (event_id, start)
//! - přesunutá schůzka = nový klíč → vystřelí znovu; zrušená zmizí
//! - po sleep/wake podmínka `now < start + 60 s` zabrání salvě
//! - víc událostí najednou → seriál (další až po zavření overlaye)

use log::{error, info};
use std::collections::{BTreeMap, HashSet};
use std::time::Duration;
use tauri::{AppHandle, Manager};

use super::calendar::eventkit;
use std::sync::atomic::{AtomicBool, Ordering};

/// Nastaví UI po změně kalendářů/oprávnění — scheduler pak na nejbližší
/// tick (do 15 s) načte kalendář znovu místo čekání na 5minutový poll.
pub static FORCE_POLL: AtomicBool = AtomicBool::new(false);

pub fn request_poll() {
    FORCE_POLL.store(true, Ordering::Relaxed);
}
use super::conf::AppConfig;
use super::window;

#[derive(Debug, Clone)]
struct FlybyJob {
    key: String, // "{event_id}@{start_ts}"
    event_id: String,
    title: String,
    start: i64,
}

pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut queue: BTreeMap<i64, Vec<FlybyJob>> = BTreeMap::new();
        let mut fired: HashSet<String> = HashSet::new();
        let mut last_poll: i64 = 0;

        loop {
            let now = chrono::Local::now().timestamp();
            let cfg = AppConfig::new();

            // --- poll: à 5 min, nebo hned po startu/wake mezeře ---
            // hodiny posunuté dozadu (ruční změna času, NTP) by jinak
            // zablokovaly pollování, dokud čas nedožene
            if now < last_poll {
                last_poll = 0;
            }
            let forced = FORCE_POLL.swap(false, Ordering::Relaxed);
            if forced || now - last_poll >= 300 {
                last_poll = now;
                let mut events = eventkit::fetch_events(24.0, &cfg.calendar_ids);
                // druhý zdroj: tajná iCal URL (pokud je nastavená)
                if cfg.ics_url_set {
                    let mut ics_events = super::calendar::ics::fetch_events(24.0).await;
                    events.append(&mut ics_events);
                }
                // Prázdný výsledek při neprázdné frontě = nejspíš výpadek
                // sítě nebo blip EventKitu, ne skutečně prázdný den.
                // Starou frontu v takovém případě ponecháme.
                if events.is_empty() && !queue.is_empty() {
                    log::warn!("Poll vrátil 0 událostí, ponechávám předchozí frontu");
                    tokio::time::sleep(Duration::from_secs(15)).await;
                    continue;
                }
                queue.clear();
                // clamp: ručně upravený JSON s 99999 by jinak vystřelil
                // všechny dnešní schůzky naráz
                let lead = i64::from(cfg.minutes_before.min(60)) * 60;
                // dedupe: kdo má stejný kalendář v EventKitu i přes ICS,
                // nesmí dostat dva přelety na tutéž schůzku
                let mut seen: HashSet<(i64, String)> = HashSet::new();
                for e in events {
                    if e.id.is_empty() {
                        continue;
                    }
                    if !seen.insert((e.start, e.title.clone())) {
                        continue;
                    }
                    let fire_at = e.start - lead;
                    let job = FlybyJob {
                        key: format!("{}@{}", e.stable_id, e.occurrence),
                        event_id: e.id,
                        title: e.title,
                        start: e.start,
                    };
                    queue.entry(fire_at).or_default().push(job);
                }
                // prune: klíče starší 24 h už nikdy nevystřelí
                fired.retain(|k| {
                    k.rsplit('@')
                        .next()
                        .and_then(|ts| ts.parse::<i64>().ok())
                        .is_some_and(|ts| now - ts < 24 * 3600)
                });
                info!(
                    "Scheduler poll: {} časů ve frontě, {} odbavených",
                    queue.len(),
                    fired.len()
                );
            }

            // --- tick ---
            let muted = cfg.mute_until.is_some_and(|t| t > now);
            if !muted && app.get_window(window::OVERLAY_LABEL).is_none() {
                let due: Vec<FlybyJob> = queue
                    .range(..=now)
                    .flat_map(|(_, jobs)| jobs.clone())
                    .collect();

                for job in due {
                    if fired.contains(&job.key) {
                        continue;
                    }
                    // prošlé schůzky tiše zahodit (sleep/wake salva)
                    if now >= job.start + 60 {
                        fired.insert(job.key);
                        continue;
                    }
                    // osobní data (název schůzky) do logu nepatří — jen délka a čas
                    // Poslední kontrola: schůzka mohla být za posledních
                    // pár minut zrušena, smazána nebo přesunuta jinam.
                    if !eventkit::event_still_valid(&job.event_id, job.start, &cfg.calendar_ids) {
                        info!("Scheduler: schůzka už neplatí (zrušena/přesunuta), přelet ruším");
                        fired.insert(job.key);
                        continue;
                    }
                    info!("Scheduler: přelet (titulek {} zn., start {})", job.title.chars().count(), job.start);
                    // fired až po úspěšném otevření okna — když se okno
                    // nepovede vytvořit, přelet se zkusí příští tick
                    if open_flyby(&app, &job) {
                        fired.insert(job.key.clone());
                    }
                    break; // max 1 přelet naráz; další chytne příští tick
                }
            }

            tokio::time::sleep(Duration::from_secs(15)).await;
        }
    });
}

fn open_flyby(app: &AppHandle, job: &FlybyJob) -> bool {
    // titulek jde do URL query → percent-encoding vlastní minimální
    // implementací (jen bezpečné znaky ponechat, zbytek %XX)
    let cfg_lang = AppConfig::new().language;
    let title = if job.title.trim().is_empty() {
        if cfg_lang == "en" { "Meeting" } else { "Schůzka" }
    } else {
        job.title.as_str()
    };
    let tidy = super::calendar::tidy_title(title);
    let title_enc = percent_encode(&tidy);
    let full_enc = percent_encode(title);
    let time = chrono::DateTime::from_timestamp(job.start, 0)
        .map(|dt| {
            dt.with_timezone(&chrono::Local)
                .format("%H:%M")
                .to_string()
        })
        .unwrap_or_default();
    // maskot dle nastavení (zvuk řeší open_overlay centrálně)
    let mascot: String = AppConfig::new()
        .mascot
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    let query = format!("mode=event&mascot={mascot}&title={title_enc}&full={full_enc}&time={time}");
    window::open_overlay(app, &query)
}

/// Minimální percent-encoding pro query hodnotu (RFC 3986 unreserved
/// ponechat, vše ostatní %XX po UTF-8 bajtech). Žádná závislost navíc.
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
