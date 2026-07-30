//! EventKit in-process (objc2) — čtení Apple Kalendáře bez sidecar
//! binárky. Jeden podepsaný Mach-O = čistší TCC i notarizace.
//! Vše read-only: appka události nikdy nevytváří ani nemění.

#![cfg(target_os = "macos")]

use log::{error, info};
use serde::Serialize;
use std::sync::mpsc;
use std::time::Duration;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::{msg_send, runtime::AnyObject};
use objc2_foundation::NSString;
use objc2_event_kit::{EKAuthorizationStatus, EKEntityType, EKEventStore};
use objc2_foundation::NSDate;

use super::sanitize_title;

/// EKEvent/EKCalendar.title je v hlavičkách nonnull, ale reálně vrací
/// nil (událost bez názvu, import bez SUMMARY). Generovaný getter by
/// v takovém případě panikařil a s `panic = "abort"` shodil celou
/// appku — proto zprávu posíláme ručně a nil ošetříme.
fn title_of<T: objc2::Message>(obj: &T) -> String {
    let s: Option<Retained<NSString>> = unsafe { msg_send![obj, title] };
    s.map(|t| t.to_string()).unwrap_or_default()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalEvent {
    pub id: String,
    pub title: String,
    /// unix timestamp začátku
    pub start: i64,
    pub calendar_id: String,
    /// stabilní identita napříč přesunem mezi kalendáři a re-syncem
    /// (calendarItemExternalIdentifier), fallback = id
    pub stable_id: String,
    /// původně plánovaný čas výskytu — u přesunuté instance série
    /// zůstává stejný, takže nevyrobí duplicitní přelet
    pub occurrence: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalInfo {
    pub id: String,
    pub title: String,
    /// "birthday" | "subscription" | "normal" — UI podle toho
    /// defaultně nezaškrtne narozeniny a odebírané kalendáře svátků
    pub kind: String,
}

/// Stav oprávnění jako string pro frontend/tray.
pub fn authorization_status() -> &'static str {
    let status = unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Event) };
    match status {
        EKAuthorizationStatus::NotDetermined => "notDetermined",
        EKAuthorizationStatus::Restricted => "restricted",
        EKAuthorizationStatus::Denied => "denied",
        EKAuthorizationStatus::FullAccess | EKAuthorizationStatus::Authorized => "authorized",
        EKAuthorizationStatus::WriteOnly => "writeOnly",
        _ => "unknown",
    }
}

/// Požádá o přístup (macOS ukáže TCC dialog právě jednou). Blokuje max
/// `timeout` — nikdy nevolat z main threadu.
pub fn request_access(timeout: Duration) -> bool {
    let store = unsafe { EKEventStore::new() };
    let (tx, rx) = mpsc::channel::<bool>();

    let block = RcBlock::new(move |granted: objc2::runtime::Bool, _err: *mut objc2_foundation::NSError| {
        let _ = tx.send(granted.as_bool());
    });

    unsafe {
        // macOS 14+: requestFullAccessToEvents (generovaná signatura chce
        // raw pointer na Block).
        let ptr = &*block as *const block2::Block<_> as *mut block2::Block<_>;
        store.requestFullAccessToEventsWithCompletion(ptr);
    }

    match rx.recv_timeout(timeout) {
        Ok(granted) => {
            info!("EventKit přístup: granted={granted}");
            granted
        }
        Err(_) => {
            error!("EventKit request_access: timeout {timeout:?}");
            false
        }
    }
}

/// Seznam kalendářů (id + název) pro nastavení.
pub fn list_calendars() -> Vec<CalInfo> {
    let store = unsafe { EKEventStore::new() };
    let cals = unsafe { store.calendarsForEntityType(EKEntityType::Event) };
    cals.iter()
        .map(|c| {
            let ctype: isize = unsafe { msg_send![&*c, type] };
            CalInfo {
                id: unsafe { c.calendarIdentifier() }.to_string(),
                title: sanitize_title(&title_of(&*c)),
                // EKCalendarType: Local=0, CalDAV=1, Exchange=2,
                // Subscription=3, Birthday=4
                kind: match ctype {
                    4 => "birthday",
                    3 => "subscription",
                    _ => "normal",
                }
                .to_string(),
            }
        })
        .collect()
}

/// Platí ještě tahle schůzka? Volá se těsně před přeletem — mezi
/// pollem a tickem mohla být zrušena, přesunuta nebo smazána.
/// Kontroluje se jen EventKit; události z ICS (id "ics:…") se ověřit
/// nedají bez dalšího síťového dotazu, ty projdou.
pub fn event_still_valid(event_id: &str, start: i64, calendar_ids: &[String]) -> bool {
    if event_id.starts_with("ics:") {
        return true;
    }
    // úzké okno kolem začátku stačí — hledáme přesně tuhle instanci
    fetch_events(2.0, calendar_ids)
        .iter()
        .any(|e| e.id == event_id && e.start == start)
}

/// Odmítl jsem tuhle pozvánku? EKParticipantStatus::Declined = 3.
/// Attendees jsou nil u schůzek bez pozvaných — pak nic nefiltrujeme.
fn is_declined_by_me(event: &objc2_event_kit::EKEvent) -> bool {
    let attendees: Option<Retained<objc2_foundation::NSArray<objc2_event_kit::EKParticipant>>> =
        unsafe { msg_send![event, attendees] };
    let Some(attendees) = attendees else {
        return false;
    };
    attendees.iter().any(|p| {
        let is_me: bool = unsafe { msg_send![&*p, isCurrentUser] };
        if !is_me {
            return false;
        }
        let status: isize = unsafe { msg_send![&*p, participantStatus] };
        status == 3
    })
}

/// Události v okně ⟨teď, teď + hours⟩, volitelně filtrované na vybrané
/// kalendáře. Opakované události EventKit rozbaluje sám.
pub fn fetch_events(hours: f64, calendar_ids: &[String]) -> Vec<CalEvent> {
    let store = unsafe { EKEventStore::new() };
    // Popostrčit sync se serverem — jinak appka čeká, až si macOS
    // sám stáhne změny z Googlu (může trvat i desítky minut).
    unsafe { store.refreshSourcesIfNecessary() };

    let start = unsafe { NSDate::now() };
    let end = unsafe { NSDate::dateWithTimeIntervalSinceNow(hours * 3600.0) };

    // None = všechny kalendáře (EventKit gotcha: ručně složené pole umí
    // tiše vracet prázdno). Filtr na vybrané kalendáře děláme až nad
    // výsledkem podle calendar_id — stejný efekt, spolehlivé chování.
    let predicate = unsafe {
        store.predicateForEventsWithStartDate_endDate_calendars(&start, &end, None)
    };
    let events = unsafe { store.eventsMatchingPredicate(&predicate) };

    let mut out: Vec<CalEvent> = events
        .iter()
        // 1) celodenní události (svátky, narozeniny, dovolené) nemají
        //    čas začátku — přelet o půlnoci nikdo nechce
        .filter(|e| !unsafe { e.isAllDay() })
        // 2) zrušené schůzky (organizátor je odvolal, EKEventStatus::Canceled = 3)
        .filter(|e| unsafe { e.status() }.0 != 3)
        // 3) pozvánky, které jsem odmítl (EKParticipantStatus::Declined = 3)
        .filter(|e| !is_declined_by_me(e))
        .filter(|e| {
            calendar_ids.is_empty()
                || unsafe { e.calendar() }
                    .map(|c| {
                        let id = unsafe { c.calendarIdentifier() }.to_string();
                        calendar_ids.iter().any(|want| want == &id)
                    })
                    .unwrap_or(false)
        })
        .filter_map(|e| {
            let start_date: Option<Retained<NSDate>> = unsafe { msg_send![&*e, startDate] };
            let start_date = start_date?;
            let title = title_of(&*e);
            let id = unsafe { e.eventIdentifier() }
                .map(|i| i.to_string())
                .unwrap_or_default();
            let ext: Option<Retained<NSString>> =
                unsafe { msg_send![&*e, calendarItemExternalIdentifier] };
            let start_ts = unsafe { start_date.timeIntervalSince1970() } as i64;
            let occ: Option<Retained<NSDate>> = unsafe { msg_send![&*e, occurrenceDate] };
            Some(CalEvent {
                stable_id: ext.map(|x| x.to_string()).unwrap_or_else(|| id.clone()),
                occurrence: occ
                    .map(|d| unsafe { d.timeIntervalSince1970() } as i64)
                    .unwrap_or(start_ts),
                id,
                title: sanitize_title(&title),
                start: start_ts,
                calendar_id: unsafe { e.calendar() }
                    .map(|c| unsafe { c.calendarIdentifier() }.to_string())
                    .unwrap_or_default(),
            })
        })
        .collect();

    out.sort_by_key(|e| e.start);
    out.truncate(500);
    out
}
