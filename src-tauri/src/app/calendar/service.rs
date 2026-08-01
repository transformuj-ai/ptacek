//! EventKitService: jediný dlouho žijící EKEventStore pro celý proces.
//!
//! Proč: každé `EKEventStore::new()` otevírá XPC spojení na calaccessd.
//! macOS má na proces limit; v0.1.4 vytvářela store při každém čtení a po
//! ~4 hodinách běhu narazila na „Client tried to open too many connections
//! to calaccessd" (EKCADErrorDomain 1021) — od té chvíle všechna čtení
//! vracela prázdno, UI lhalo „nejsou kalendáře" a scheduler mazal frontu.
//!
//! Architektura: jedno vyhrazené OS vlákno vlastní store a zpracovává
//! požadavky z kanálu serializovaně. Každá operace běží v autoreleasepool
//! (tokio vlákna žádný nemají — ObjC objekty by se nikdy neuvolnily).
//! Chyba se nikdy nevydává za prázdný kalendář: čtení vrací Result a
//! service drží health snapshot pro UI. Nefunkční store se obnovuje
//! kontrolovaně (bounded reset s backoffem), ne při každém čtení.

#![cfg(target_os = "macos")]

use log::{info, warn};
use serde::Serialize;
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;

use objc2::rc::{autoreleasepool, Retained};
use objc2::runtime::AnyObject;
use objc2::msg_send;
use objc2_event_kit::EKEventStore;

use super::eventkit::{self, CalEvent, CalInfo};

/// Výsledek finální kontroly schůzky těsně před přeletem.
/// `Unknown` = službě se nedařilo číst — NENÍ to důkaz zrušení.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventValidity {
    Valid,
    Gone,
    Unknown,
}

/// Pravdivý stav kalendářové vrstvy pro UI a diagnostiku.
/// Bez názvů kalendářů a schůzek — jen čísla a kódy.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarHealth {
    pub status: String,
    pub calendars: u32,
    pub last_success: Option<i64>,
    pub last_error: Option<String>,
    pub consecutive_failures: u32,
    pub store_generation: u32,
}

impl CalendarHealth {
    fn initial() -> Self {
        CalendarHealth {
            status: eventkit::authorization_status().to_string(),
            calendars: 0,
            last_success: None,
            last_error: None,
            consecutive_failures: 0,
            store_generation: 0,
        }
    }
}

enum Request {
    Access {
        timeout: Duration,
        reply: mpsc::Sender<bool>,
    },
    ListCalendars {
        reply: mpsc::Sender<Result<Vec<CalInfo>, String>>,
    },
    FetchEvents {
        hours: f64,
        calendar_ids: Vec<String>,
        reply: mpsc::Sender<Result<Vec<CalEvent>, String>>,
    },
    Validate {
        event_id: String,
        start: i64,
        calendar_ids: Vec<String>,
        reply: mpsc::Sender<EventValidity>,
    },
    Health {
        reply: mpsc::Sender<CalendarHealth>,
    },
}

static SENDER: Mutex<Option<mpsc::Sender<Request>>> = Mutex::new(None);

/// Spustí service vlákno. Volat jednou při startu appky, před schedulerem.
pub fn start(app: tauri::AppHandle) {
    let (tx, rx) = mpsc::channel::<Request>();
    *SENDER.lock().unwrap() = Some(tx);
    std::thread::Builder::new()
        .name("eventkit-service".into())
        .spawn(move || worker_loop(rx, app))
        .expect("eventkit-service vlákno se nepodařilo spustit");
}

fn send(req: Request) -> bool {
    match SENDER.lock().unwrap().as_ref() {
        Some(tx) => tx.send(req).is_ok(),
        None => false,
    }
}

/// TCC dialog + po grantu čerstvý store. Blokuje max `timeout` + rezervu.
pub fn request_access(timeout: Duration) -> bool {
    let (tx, rx) = mpsc::channel();
    if !send(Request::Access { timeout, reply: tx }) {
        return false;
    }
    rx.recv_timeout(timeout + Duration::from_secs(10))
        .unwrap_or(false)
}

pub fn list_calendars() -> Result<Vec<CalInfo>, String> {
    let (tx, rx) = mpsc::channel();
    if !send(Request::ListCalendars { reply: tx }) {
        return Err("service-unavailable".into());
    }
    rx.recv_timeout(Duration::from_secs(30))
        .unwrap_or_else(|_| Err("timeout".into()))
}

pub fn fetch_events(hours: f64, calendar_ids: &[String]) -> Result<Vec<CalEvent>, String> {
    let (tx, rx) = mpsc::channel();
    if !send(Request::FetchEvents {
        hours,
        calendar_ids: calendar_ids.to_vec(),
        reply: tx,
    }) {
        return Err("service-unavailable".into());
    }
    rx.recv_timeout(Duration::from_secs(30))
        .unwrap_or_else(|_| Err("timeout".into()))
}

/// Platí ještě schůzka? ICS události (id „ics:…") ověřit nejdou → Valid.
pub fn event_still_valid(event_id: &str, start: i64, calendar_ids: &[String]) -> EventValidity {
    if event_id.starts_with("ics:") {
        return EventValidity::Valid;
    }
    let (tx, rx) = mpsc::channel();
    if !send(Request::Validate {
        event_id: event_id.to_string(),
        start,
        calendar_ids: calendar_ids.to_vec(),
        reply: tx,
    }) {
        return EventValidity::Unknown;
    }
    rx.recv_timeout(Duration::from_secs(30))
        .unwrap_or(EventValidity::Unknown)
}

pub fn health() -> CalendarHealth {
    let (tx, rx) = mpsc::channel();
    if !send(Request::Health { reply: tx }) {
        return CalendarHealth::initial();
    }
    rx.recv_timeout(Duration::from_secs(5))
        .unwrap_or_else(|_| CalendarHealth::initial())
}

// ---------------------------------------------------------------------------
// worker
// ---------------------------------------------------------------------------

/// Bounded recovery: minimální rozestup resetů a strop za hodinu. Reset je
/// záchrana pro prokázaně mrtvý store, ne pracovní režim.
struct ResetBudget {
    last_reset: i64,
    window_start: i64,
    in_window: u32,
}

impl ResetBudget {
    fn new() -> Self {
        ResetBudget {
            last_reset: 0,
            window_start: 0,
            in_window: 0,
        }
    }

    fn allow(&mut self, now: i64) -> bool {
        if now - self.window_start >= 3600 {
            self.window_start = now;
            self.in_window = 0;
        }
        if now - self.last_reset < 60 || self.in_window >= 5 {
            return false;
        }
        self.last_reset = now;
        self.in_window += 1;
        true
    }
}

struct Worker {
    /// None jen v unit testech — emit se pak přeskočí
    app: Option<tauri::AppHandle>,
    store: Option<Retained<EKEventStore>>,
    observer_token: Option<Retained<AnyObject>>,
    health: CalendarHealth,
    /// nejvíc kalendářů, co jsme kdy v tomhle procesu viděli — náhlá nula
    /// při authorized je pak signál mrtvého store, ne prázdného systému
    max_cals_seen: u32,
    resets: ResetBudget,
}

fn worker_loop(rx: mpsc::Receiver<Request>, app: tauri::AppHandle) {
    let mut w = Worker {
        app: Some(app),
        store: None,
        observer_token: None,
        health: CalendarHealth::initial(),
        max_cals_seen: 0,
        resets: ResetBudget::new(),
    };
    while let Ok(req) = rx.recv() {
        autoreleasepool(|_| w.handle(req));
    }
}

impl Worker {
    fn store(&mut self) -> &EKEventStore {
        if self.store.is_none() {
            let store = unsafe { EKEventStore::new() };
            self.health.store_generation += 1;
            info!(
                "EventKit service: store vytvořen (generace {})",
                self.health.store_generation
            );
            self.register_observer(&store);
            self.store = Some(store);
        }
        self.store.as_ref().unwrap()
    }

    /// Kontrolovaná výměna store (po grantu nebo při prokázaném mrtvém
    /// stavu). Observer se přepojí na novou instanci.
    fn recreate_store(&mut self, reason: &str) {
        info!("EventKit service: obnovuji store ({reason})");
        self.unregister_observer();
        self.store = None;
        let _ = self.store();
    }

    /// EKEventStoreChangedNotification → okamžitý refresh scheduleru.
    /// Poll à 5 min zůstává jen jako pojistka.
    fn register_observer(&mut self, store: &EKEventStore) {
        use block2::RcBlock;
        use core::ptr::NonNull;
        use objc2_foundation::{NSNotification, NSNotificationCenter, NSOperationQueue};

        let block = RcBlock::new(move |_n: NonNull<NSNotification>| {
            // debounce řeší FORCE_POLL (idempotentní) + notify scheduleru
            crate::app::scheduler::request_poll_now();
        });
        let center = NSNotificationCenter::defaultCenter();
        let queue = NSOperationQueue::mainQueue();
        let name = unsafe { objc2_event_kit::EKEventStoreChangedNotification };
        let token: Retained<AnyObject> = unsafe {
            msg_send![
                &*center,
                addObserverForName: name,
                object: store,
                queue: &*queue,
                usingBlock: &*block,
            ]
        };
        // token drží registraci; block si NotificationCenter kopíruje sám
        self.observer_token = Some(token);
    }

    fn unregister_observer(&mut self) {
        use objc2_foundation::NSNotificationCenter;
        if let Some(token) = self.observer_token.take() {
            let center = NSNotificationCenter::defaultCenter();
            let _: () = unsafe { msg_send![&*center, removeObserver: &*token] };
        }
    }

    fn emit_health(&self) {
        use tauri::Manager;
        if let Some(app) = &self.app {
            let _ = app.emit_all("ptacek://calendar-health", &self.health);
        }
    }

    fn success(&mut self, calendars: u32) {
        self.health.status = eventkit::authorization_status().to_string();
        self.health.calendars = calendars;
        self.health.last_success = Some(chrono::Local::now().timestamp());
        self.health.last_error = None;
        self.health.consecutive_failures = 0;
        self.max_cals_seen = self.max_cals_seen.max(calendars);
        self.emit_health();
    }

    fn failure(&mut self, code: &str) {
        self.health.status = eventkit::authorization_status().to_string();
        self.health.last_error = Some(code.to_string());
        self.health.consecutive_failures += 1;
        warn!(
            "EventKit service: čtení selhalo (kód={code}, v řadě {})",
            self.health.consecutive_failures
        );
        self.emit_health();
    }

    fn handle(&mut self, req: Request) {
        match req {
            Request::Access { timeout, reply } => {
                let granted = eventkit::request_access(self.store(), timeout);
                if granted {
                    // store vzniklý před grantem nemusí oprávnění vidět —
                    // jednou ho vyměnit je levné a deterministické
                    self.recreate_store("po udělení přístupu");
                }
                self.health.status = eventkit::authorization_status().to_string();
                self.emit_health();
                let _ = reply.send(granted);
            }
            Request::ListCalendars { reply } => {
                let result = self.list_with_recovery();
                if let Ok(cals) = &result {
                    let cals = cals.clone();
                    self.report_unavailable_selection(&cals);
                }
                let _ = reply.send(result);
            }
            Request::FetchEvents {
                hours,
                calendar_ids,
                reply,
            } => {
                let _ = reply.send(self.fetch_with_recovery(hours, &calendar_ids));
            }
            Request::Validate {
                event_id,
                start,
                calendar_ids,
                reply,
            } => {
                let verdict = match self.fetch_with_recovery(2.0, &calendar_ids) {
                    Ok(events) => {
                        if events.iter().any(|e| e.id == event_id && e.start == start) {
                            EventValidity::Valid
                        } else {
                            EventValidity::Gone
                        }
                    }
                    Err(_) => EventValidity::Unknown,
                };
                let _ = reply.send(verdict);
            }
            Request::Health { reply } => {
                self.health.status = eventkit::authorization_status().to_string();
                let _ = reply.send(self.health.clone());
            }
        }
    }

    fn list_with_recovery(&mut self) -> Result<Vec<CalInfo>, String> {
        if eventkit::authorization_status() != "authorized" {
            self.failure("no-permission");
            return Err("no-permission".into());
        }
        let cals = eventkit::list_calendars(self.store());
        if !cals.is_empty() {
            self.success(cals.len() as u32);
            return Ok(cals);
        }
        // Nula kalendářů při authorized: buď fakt prázdný systém, nebo
        // mrtvý store. Jeden kontrolovaný reset to rozsoudí.
        let now = chrono::Local::now().timestamp();
        if self.resets.allow(now) {
            self.recreate_store("authorized, ale 0 kalendářů");
            let retry = eventkit::list_calendars(self.store());
            if !retry.is_empty() {
                info!("EventKit service: reset store pomohl, kalendáře jsou zpět");
                self.success(retry.len() as u32);
                return Ok(retry);
            }
        }
        if self.max_cals_seen > 0 {
            // dřív jsme kalendáře viděli → tohle je výpadek, ne prázdno
            self.failure("service-unavailable");
            Err("service-unavailable".into())
        } else {
            // dvakrát potvrzené prázdno u čerstvého procesu = pravda
            self.success(0);
            Ok(cals)
        }
    }

    fn fetch_with_recovery(
        &mut self,
        hours: f64,
        calendar_ids: &[String],
    ) -> Result<Vec<CalEvent>, String> {
        if eventkit::authorization_status() != "authorized" {
            self.failure("no-permission");
            return Err("no-permission".into());
        }
        let events = eventkit::fetch_events(self.store(), hours, calendar_ids);
        if !events.is_empty() {
            let cals = eventkit::calendar_count(self.store());
            self.success(cals);
            return Ok(events);
        }
        // Prázdný výsledek je legitimní (žádné schůzky). Mrtvý store se
        // pozná podle kalendářů: měli jsme je, a najednou nejsou.
        let cals = eventkit::calendar_count(self.store());
        if cals == 0 && self.max_cals_seen > 0 {
            let now = chrono::Local::now().timestamp();
            if self.resets.allow(now) {
                self.recreate_store("fetch: kalendáře zmizely");
                let retry = eventkit::fetch_events(self.store(), hours, calendar_ids);
                let cals_retry = eventkit::calendar_count(self.store());
                if cals_retry > 0 {
                    info!("EventKit service: reset store pomohl");
                    self.success(cals_retry);
                    return Ok(retry);
                }
            }
            self.failure("service-unavailable");
            return Err("service-unavailable".into());
        }
        self.success(cals);
        Ok(events)
    }

    /// Uložený výběr kalendářů může obsahovat ID, která právě nejsou
    /// vidět (odebraný účet, výpadek syncu, nová UUID po re-přidání).
    /// NIC nemažeme — kalendář mohl zmizet jen dočasně. Jen to nahlásíme
    /// UI, ať je umí označit jako momentálně nedostupné; odstranit je smí
    /// jedině uživatel vědomou akcí.
    fn report_unavailable_selection(&mut self, cals: &[CalInfo]) {
        if cals.is_empty() {
            return;
        }
        let selected = crate::app::conf::AppConfig::new().calendar_ids;
        let Some(app) = &self.app else { return };
        if selected.is_empty() {
            return;
        }
        let missing = selected
            .iter()
            .filter(|id| !cals.iter().any(|c| &c.id == *id))
            .count() as u32;
        if missing > 0 {
            warn!(
                "EventKit service: {missing} vybraných kalendářů teď není vidět (nechávám ve výběru)"
            );
        }
        use tauri::Manager;
        let _ = app.emit_all("ptacek://calendar-selection-unavailable", missing);
    }
}

#[cfg(test)]
mod tests {
    use super::{CalendarHealth, ResetBudget, Worker};

    /// Jádro opravy incidentu 1021: opakovaný přístup ke store NESMÍ
    /// vytvářet další instance (v0.1.4 vytvářela store při každém čtení
    /// a po ~50 čteních vyčerpala spojení na calaccessd).
    #[test]
    fn tisic_pristupu_ke_store_vytvori_jedinou_instanci() {
        let mut w = Worker {
            app: None,
            store: None,
            observer_token: None,
            health: CalendarHealth::initial(),
            max_cals_seen: 0,
            resets: ResetBudget::new(),
        };
        for _ in 0..1000 {
            objc2::rc::autoreleasepool(|_| {
                let _ = w.store();
            });
        }
        assert_eq!(
            w.health.store_generation, 1,
            "1000 operací = právě jeden store, žádné další instance"
        );
    }

    #[test]
    fn reset_budget_dodrzi_minimalni_rozestup() {
        let mut b = ResetBudget::new();
        assert!(b.allow(1000));
        assert!(!b.allow(1030), "reset dřív než za 60 s neprojde");
        assert!(b.allow(1061));
    }

    #[test]
    fn reset_budget_strop_pet_za_hodinu() {
        let mut b = ResetBudget::new();
        let mut t = 1000;
        for _ in 0..5 {
            assert!(b.allow(t));
            t += 61;
        }
        assert!(!b.allow(t), "šestý reset v hodině neprojde");
        assert!(b.allow(t + 3600), "nové okno = nový rozpočet");
    }
}
