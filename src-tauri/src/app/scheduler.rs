//! Scheduler přeletů: poll kalendáře → in-memory fronta → tick.
//! Runtime stav žije jen tady (Rust); frontend je hloupý renderer.
//!
//! Pravidla (dle plánu):
//! - poll à 5 min (+ hned po startu), tick à 15 s
//! - přelet v čase `start - minutes_before`, max 1× na (event_id, start)
//! - přesunutá schůzka = nový klíč → vystřelí znovu; zrušená zmizí
//! - po sleep/wake podmínka `now < start + 60 s` zabrání salvě
//! - víc událostí najednou → seriál (další až po zavření overlaye)

use log::info;
use std::collections::{BTreeMap, HashSet};
use std::time::Duration;
use tauri::{AppHandle, Manager};

use super::calendar::eventkit;
use super::calendar::service::{self, EventValidity};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

/// Nastaví UI po změně kalendářů/oprávnění — scheduler pak na nejbližší
/// tick (do 15 s) načte kalendář znovu místo čekání na 5minutový poll.
pub static FORCE_POLL: AtomicBool = AtomicBool::new(false);

/// Probuzení smyčky mimo 15s tick (EKEventStoreChangedNotification).
/// AtomicBool je zároveň přirozený debounce: burst notifikací = jeden poll.
static WAKE: OnceLock<tokio::sync::Notify> = OnceLock::new();

fn wake() -> &'static tokio::sync::Notify {
    WAKE.get_or_init(tokio::sync::Notify::new)
}

pub fn request_poll() {
    FORCE_POLL.store(true, Ordering::Relaxed);
}

/// Jako request_poll, ale probudí smyčku hned — reakce na změnu
/// kalendáře do pár vteřin místo čekání na tick.
pub fn request_poll_now() {
    request_poll();
    wake().notify_one();
}
use super::conf::AppConfig;
use super::window;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Source {
    Ekit,
    Ics,
}

#[derive(Clone)]
struct FlybyJob {
    key: String, // "{stable_id}@{occurrence}"
    event_id: String,
    title: String,
    start: i64,
    source: Source,
}

/// Přestaví frontu po pollu. Každý zdroj si spravuje jen svoje joby a
/// pro OBA platí stejný princip (poučení z incidentu 1021):
/// - úspěch nahrazuje VŽDY, i prázdnem (smazaná událost musí z fronty),
/// - chyba čtení NIKDY nemaže — předchozí joby zdroje se ponechají.
///   Výpadek služby nesmí zahodit dnešní schůzky.
///
/// Dedupe: v rámci zdroje podle klíče (stable_id@occurrence — dvě různé
/// schůzky se stejným názvem a časem přežijí), mezi zdroji heuristika
/// (start, titulek) a ICS ustupuje EventKitu — kdo má tentýž kalendář
/// připojený oběma cestami, nedostane dva přelety na jednu schůzku.
fn rebuild_queue(
    prev: &BTreeMap<i64, Vec<FlybyJob>>,
    ekit: Result<Vec<eventkit::CalEvent>, String>,
    ics: Option<Result<Vec<eventkit::CalEvent>, String>>,
    lead: i64,
) -> BTreeMap<i64, Vec<FlybyJob>> {
    let mut queue: BTreeMap<i64, Vec<FlybyJob>> = BTreeMap::new();
    let mut keys: HashSet<String> = HashSet::new();
    let mut cross: HashSet<(i64, String)> = HashSet::new();

    let carry_over = |queue: &mut BTreeMap<i64, Vec<FlybyJob>>,
                      keys: &mut HashSet<String>,
                      source: Source| {
        for job in prev.values().flatten().filter(|j| j.source == source) {
            if keys.insert(job.key.clone()) {
                queue
                    .entry(job.start - lead)
                    .or_default()
                    .push(job.clone());
            }
        }
    };

    let add = |queue: &mut BTreeMap<i64, Vec<FlybyJob>>,
                   keys: &mut HashSet<String>,
                   cross: &mut HashSet<(i64, String)>,
                   e: &eventkit::CalEvent,
                   source: Source,
                   cross_check: bool| {
        if e.id.is_empty() {
            return;
        }
        let key = format!("{}@{}", e.stable_id, e.occurrence);
        if !keys.insert(key.clone()) {
            return; // duplicitní záznam téže instance
        }
        if cross_check && !cross.insert((e.start, e.title.clone())) {
            return; // stejná schůzka už je z druhého zdroje
        }
        if !cross_check {
            cross.insert((e.start, e.title.clone()));
        }
        queue.entry(e.start - lead).or_default().push(FlybyJob {
            key,
            event_id: e.id.clone(),
            title: e.title.clone(),
            start: e.start,
            source,
        });
    };

    match &ekit {
        Ok(events) => {
            for e in events {
                add(&mut queue, &mut keys, &mut cross, e, Source::Ekit, false);
            }
        }
        // chyba čtení → přenést předchozí EventKit joby beze změny;
        // degraded služba nesmí zahodit naplánované schůzky
        Err(_) => carry_over(&mut queue, &mut keys, Source::Ekit),
    }
    match ics {
        // ICS vypnuté → žádné ICS joby
        None => {}
        Some(Ok(events)) => {
            for e in &events {
                add(&mut queue, &mut keys, &mut cross, e, Source::Ics, true);
            }
        }
        // chyba fetche → přenést předchozí ICS joby beze změny
        Some(Err(_)) => carry_over(&mut queue, &mut keys, Source::Ics),
    }
    queue
}

pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut queue: BTreeMap<i64, Vec<FlybyJob>> = BTreeMap::new();
        let mut fired: HashSet<String> = HashSet::new();
        // finální kontrola vrátila Unknown → počítáme krátké retry,
        // po kterých schůzka radši letí z posledního potvrzeného snapshotu
        let mut unknown_retries: std::collections::HashMap<String, u8> =
            std::collections::HashMap::new();
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
                let ekit = service::fetch_events(24.0, &cfg.calendar_ids);
                let ekit_desc = match &ekit {
                    Ok(events) => format!("ok událostí={}", events.len()),
                    Err(code) => format!("CHYBA kód={code}, nechávám předchozí joby"),
                };
                // druhý zdroj: tajná iCal URL (pokud je nastavená)
                let ics = if cfg.ics_url_set {
                    let r = super::calendar::ics::fetch_events(24.0).await;
                    if let Err(e) = &r {
                        log::warn!("ICS poll selhal ({e}), nechávám předchozí ICS joby");
                    }
                    Some(r)
                } else {
                    None
                };
                // clamp: ručně upravený JSON s 99999 by jinak vystřelil
                // všechny dnešní schůzky naráz
                let lead = i64::from(cfg.minutes_before.min(60)) * 60;
                queue = rebuild_queue(&queue, ekit, ics, lead);
                // prune: klíče starší 24 h už nikdy nevystřelí
                fired.retain(|k| {
                    k.rsplit('@')
                        .next()
                        .and_then(|ts| ts.parse::<i64>().ok())
                        .is_some_and(|ts| now - ts < 24 * 3600)
                });
                unknown_retries.retain(|k, _| !fired.contains(k));
                // strukturovaně a bez osobních dat: zdroje, počty, fronta
                info!(
                    "Scheduler poll: ekit {} · fronta={} odbaveno={}",
                    ekit_desc,
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
                    // Poslední kontrola: schůzka mohla být za posledních
                    // pár minut zrušena, smazána nebo přesunuta jinam.
                    // Unknown ≠ zrušena: výpadek čtení nesmí schůzku
                    // potichu zahodit — po 2 pokusech letí z posledního
                    // potvrzeného snapshotu (fronta se staví jen z Ok pollů).
                    match service::event_still_valid(&job.event_id, job.start, &cfg.calendar_ids) {
                        EventValidity::Gone => {
                            info!("Scheduler: schůzka už neplatí (zrušena/přesunuta), přelet ruším");
                            fired.insert(job.key);
                            continue;
                        }
                        EventValidity::Unknown => {
                            let tries = unknown_retries.entry(job.key.clone()).or_insert(0);
                            *tries += 1;
                            if *tries <= 2 {
                                info!(
                                    "Scheduler: ověření schůzky selhalo (pokus {}), zkusím za tick",
                                    tries
                                );
                                break; // neoznačovat fired, příští tick znovu
                            }
                            log::warn!(
                                "Scheduler: ověření nedostupné, letím z posledního potvrzeného snapshotu"
                            );
                        }
                        EventValidity::Valid => {}
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

            // tick à 15 s, ale notifikace změny kalendáře probudí hned
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(15)) => {}
                _ = wake().notified() => {}
            }
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

#[cfg(test)]
mod queue_tests {
    use super::*;

    fn ev(id: &str, start: i64, title: &str) -> eventkit::CalEvent {
        eventkit::CalEvent {
            stable_id: id.to_string(),
            occurrence: start,
            id: id.to_string(),
            title: title.to_string(),
            start,
            calendar_id: "test".to_string(),
        }
    }

    fn jobs(q: &BTreeMap<i64, Vec<FlybyJob>>) -> Vec<&FlybyJob> {
        q.values().flatten().collect()
    }

    #[test]
    fn vypadek_ics_necha_predchozi_ics_joby() {
        let prev = rebuild_queue(
            &BTreeMap::new(),
            Ok(vec![]),
            Some(Ok(vec![ev("ics:a", 1000, "Call")])),
            60,
        );
        assert_eq!(jobs(&prev).len(), 1);
        // další poll: ICS spadne → job přežije
        let q = rebuild_queue(&prev, Ok(vec![]), Some(Err("timeout".into())), 60);
        assert_eq!(jobs(&q).len(), 1, "výpadek sítě nesmí zahodit schůzku");
    }

    #[test]
    fn uspesne_prazdne_ics_frontu_vycisti() {
        let prev = rebuild_queue(
            &BTreeMap::new(),
            Ok(vec![]),
            Some(Ok(vec![ev("ics:a", 1000, "Zrušený call")])),
            60,
        );
        let q = rebuild_queue(&prev, Ok(vec![]), Some(Ok(Vec::new())), 60);
        assert!(jobs(&q).is_empty(), "smazaná ICS událost musí z fronty pryč");
    }

    #[test]
    fn chyba_eventkitu_zachova_predchozi_eventkit_joby() {
        let prev = rebuild_queue(
            &BTreeMap::new(),
            Ok(vec![ev("e1", 1000, "Porada"), ev("e2", 2000, "Standup")]),
            None,
            60,
        );
        assert_eq!(jobs(&prev).len(), 2);
        // výpadek služby (1021 apod.) → schůzky přežijí
        let q = rebuild_queue(&prev, Err("service-unavailable".into()), None, 60);
        assert_eq!(
            jobs(&q).len(),
            2,
            "výpadek EventKitu nesmí zahodit naplánované schůzky"
        );
    }

    #[test]
    fn uspesne_prazdny_eventkit_frontu_vycisti() {
        let prev = rebuild_queue(&BTreeMap::new(), Ok(vec![ev("e1", 1000, "A")]), None, 60);
        let q = rebuild_queue(&prev, Ok(vec![]), None, 60);
        assert!(
            jobs(&q).is_empty(),
            "potvrzené prázdno = smazaná schůzka musí z fronty"
        );
    }

    #[test]
    fn ics_funguje_i_pri_degradovanem_eventkitu() {
        let q = rebuild_queue(
            &BTreeMap::new(),
            Err("service-unavailable".into()),
            Some(Ok(vec![ev("ics:a", 1000, "Call")])),
            60,
        );
        let j = jobs(&q);
        assert_eq!(j.len(), 1, "ICS zdroj jede nezávisle na EventKitu");
        assert_eq!(j[0].source, Source::Ics);
    }

    #[test]
    fn dve_schuzky_stejny_nazev_i_cas_prezijou_v_ramci_zdroje() {
        let q = rebuild_queue(
            &BTreeMap::new(),
            Ok(vec![ev("e1", 1000, "Standup"), ev("e2", 1000, "Standup")]),
            None,
            60,
        );
        assert_eq!(jobs(&q).len(), 2, "různé schůzky nesmí splynout kvůli názvu");
    }

    #[test]
    fn stejna_schuzka_z_obou_zdroju_leti_jednou() {
        let q = rebuild_queue(
            &BTreeMap::new(),
            Ok(vec![ev("ekit-1", 1000, "Porada")]),
            Some(Ok(vec![ev("ics:x", 1000, "Porada")])),
            60,
        );
        let j = jobs(&q);
        assert_eq!(j.len(), 1);
        assert_eq!(j[0].source, Source::Ekit, "ICS ustupuje EventKitu");
    }

    #[test]
    fn fire_at_respektuje_lead() {
        let q = rebuild_queue(&BTreeMap::new(), Ok(vec![ev("e1", 1000, "A")]), None, 300);
        assert_eq!(*q.keys().next().unwrap(), 700);
    }

    #[test]
    fn duplicitni_zaznam_teze_instance_se_slouci() {
        let q = rebuild_queue(
            &BTreeMap::new(),
            Ok(vec![ev("e1", 1000, "A"), ev("e1", 1000, "A")]),
            None,
            60,
        );
        assert_eq!(jobs(&q).len(), 1);
    }

    #[test]
    fn request_poll_je_idempotentni_debounce() {
        use std::sync::atomic::Ordering;
        // burst notifikací → jediný spotřebovaný poll
        request_poll();
        request_poll();
        request_poll();
        assert!(FORCE_POLL.swap(false, Ordering::Relaxed));
        assert!(!FORCE_POLL.swap(false, Ordering::Relaxed), "druhý swap už nic nemá");
    }
}
