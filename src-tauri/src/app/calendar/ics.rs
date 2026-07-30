//! ICS fallback: tajná iCal URL (Google/Outlook ji generují) → čtení
//! událostí bez OAuth a bez připojování účtu do macOS.
//!
//! Bezpečnostní mantinely (závazné, viz plán):
//! - URL jen HTTPS, ne privátní IP/localhost, port 443/8443
//! - URL je tajemství (nese token) → macOS Keychain, nikdy settings.json
//! - fetch: timeout 15 s / connect 5 s, redirecty max 3 (jen https a jen
//!   tentýž host), DNS resolve 1× s pinem adres (žádný rebinding),
//!   streamovaný limit 5 MB po dekompresi
//! - parser: tolerantní (vadný VEVENT se přeskočí); SUMMARY/DTSTART/UID/
//!   STATUS/RRULE/EXDATE/RECURRENCE-ID; TZID přes chrono-tz; celodenní
//!   se přeskakují; max 500 událostí, RRULE strop 100 instancí
//! - texty procházejí stejnou sanitizací jako EventKit

use futures_util::StreamExt;
use log::{error, info, warn};
use std::io::BufReader;
use std::net::IpAddr;
use std::time::Duration;

use super::eventkit::CalEvent;
use super::sanitize_title;

const KEYCHAIN_SERVICE: &str = "ai.transformuj.ptacek";
const KEYCHAIN_ACCOUNT: &str = "ics-url";
const MAX_BYTES: usize = 5 * 1024 * 1024;
const MAX_EVENTS: usize = 500;

// --- Keychain ---

pub fn get_url() -> Option<String> {
    keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .ok()?
        .get_password()
        .ok()
}

pub fn set_url(url: &str) -> Result<(), String> {
    validate_url(url)?;
    keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .map_err(|e| e.to_string())?
        .set_password(url)
        .map_err(|e| e.to_string())
}

pub fn clear_url() {
    if let Ok(entry) = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT) {
        let _ = entry.delete_credential();
    }
}

/// Validace při uložení: jen https, žádný localhost/privátní IP,
/// port 443/8443. URL se nikdy neloguje (nese token).
/// Odmítne IP adresy, na které se nesmí připojovat (loopback, privátní
/// sítě, link-local, multicast) — obrana proti DNS rebindingu.
fn is_forbidden_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_multicast()
                || v4.is_broadcast()
                || v4.is_unspecified()
                || v4.octets()[0] == 0
                // CGNAT 100.64.0.0/10
                || (v4.octets()[0] == 100 && (64..=127).contains(&v4.octets()[1]))
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_multicast()
                || v6.is_unspecified()
                // unique local fc00::/7 a link-local fe80::/10
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
                || v6.to_ipv4_mapped().is_some_and(|m| is_forbidden_ip(&IpAddr::V4(m)))
        }
    }
}

/// Resolve + kontrola všech adres v jednom kroku. Vrací ověřené adresy,
/// aby je fetch mohl PŘIPNOUT ke spojení — bez toho by druhý resolve
/// uvnitř HTTP klienta mohl dostat jinou (už privátní) odpověď a obejít
/// kontrolu (DNS rebinding, TOCTOU).
fn resolve_validated(host: &str, port: u16) -> Result<Vec<std::net::SocketAddr>, String> {
    use std::net::ToSocketAddrs;
    let addrs: Vec<std::net::SocketAddr> = (host, port)
        .to_socket_addrs()
        .map_err(|_| "Doménu se nepodařilo přeložit".to_string())?
        .collect();
    if addrs.is_empty() {
        return Err("Doménu se nepodařilo přeložit".into());
    }
    if addrs.iter().any(|a| is_forbidden_ip(&a.ip())) {
        return Err("Adresa směřuje do lokální sítě, to není povolené".into());
    }
    Ok(addrs)
}

/// Syntaktická část validace — bez DNS, deterministická (testovatelná
/// offline). Vrací (host, port) pro navazující resolve.
fn validate_url_syntax(raw: &str) -> Result<(String, u16), String> {
    let url = reqwest::Url::parse(raw).map_err(|_| "Neplatná adresa".to_string())?;
    if url.scheme() != "https" {
        return Err("Adresa musí začínat https://".into());
    }
    match url.port_or_known_default() {
        Some(443) | Some(8443) => {}
        _ => return Err("Povolený port je jen 443 nebo 8443".into()),
    }
    let Some(host) = url.host_str() else {
        return Err("Adresa nemá doménu".into());
    };
    let host = host.trim_matches(['[', ']']).to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".local") {
        return Err("Lokální adresy nejsou povolené".into());
    }
    if host.parse::<IpAddr>().is_ok() {
        return Err("IP adresy nejsou povolené, použij doménu".into());
    }
    let port = url.port_or_known_default().unwrap_or(443);
    Ok((host, port))
}

pub fn validate_url(raw: &str) -> Result<(), String> {
    let (host, port) = validate_url_syntax(raw)?;
    resolve_validated(&host, port)?;
    Ok(())
}

// --- Fetch + parse ---

/// Chyba fetche je jiná informace než prázdný kalendář: scheduler si při
/// chybě nechá starou frontu, při prázdném úspěchu ji vyčistí; UI při
/// chybě řekne pravdu místo „0 událostí".
pub async fn fetch_events(hours: f64) -> Result<Vec<CalEvent>, String> {
    let Some(url) = get_url() else {
        return Err("keychain".into());
    };
    // fail-closed: hodnota z Klíčenky se validuje znovu (mohla ji tam
    // zapsat jiná cesta než naše UI)
    let (host, port) = match validate_url_syntax(&url) {
        Ok(hp) => hp,
        Err(_) => {
            warn!("ICS: uložená adresa neprošla validací, přeskakuji");
            return Err("invalid-url".into());
        }
    };
    // Resolve PRÁVĚ JEDNOU a ověřené adresy připnout ke klientovi.
    // Klient pak nikdy neresolvuje sám → DNS odpověď se mezi kontrolou
    // a spojením nemůže vyměnit za privátní IP (rebinding/TOCTOU).
    let pinned = match resolve_validated(&host, port) {
        Ok(a) => a,
        Err(e) => {
            warn!("ICS: {e}");
            return Err("dns".into());
        }
    };

    let redirect_host = host.clone();
    let client = match reqwest::Client::builder()
        .https_only(true)
        .resolve_to_addrs(&host, &pinned)
        // Přesměrování jen v rámci téhož hostu: pin adres platí per
        // doména, cizí doména by resolvovala mimo kontrolu (SSRF).
        // Google ICS feedy cross-host neredirectují, takže to nikoho
        // legitimního neomezí.
        .redirect(reqwest::redirect::Policy::custom(move |attempt| {
            if attempt.previous().len() >= 3 {
                return attempt.error("příliš mnoho přesměrování");
            }
            let same_host = attempt.url().host_str().map(str::to_ascii_lowercase)
                == Some(redirect_host.clone());
            if !same_host {
                return attempt.error("přesměrování na cizí doménu");
            }
            match validate_url_syntax(attempt.url().as_str()) {
                Ok(_) => attempt.follow(),
                Err(_) => attempt.error("přesměrování na nepovolenou adresu"),
            }
        }))
        // žádná systémová/env proxy — tajná URL s tokenem nesmí jít
        // přes cizí prostředníka
        .no_proxy()
        .timeout(Duration::from_secs(15))
        .connect_timeout(Duration::from_secs(5))
        .user_agent("TransformujPtacek/0.1 (+https://transformuj.ai)")
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            error!("ICS klient se nepodařilo vytvořit: {e}");
            return Err("client".into());
        }
    };

    let resp = match client.get(url).send().await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            warn!("ICS feed vrátil status {}", r.status());
            return Err(format!("HTTP {}", r.status().as_u16()));
        }
        Err(e) => {
            // e může obsahovat URL s tokenem → logovat jen druh chyby
            let kind = if e.is_timeout() {
                "timeout"
            } else if e.is_connect() {
                "connect"
            } else {
                "download"
            };
            warn!("ICS fetch selhal ({kind})");
            return Err(kind.into());
        }
    };

    // streamovaný limit velikosti PO dekompresi
    let mut body: Vec<u8> = Vec::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(bytes) => {
                if body.len() + bytes.len() > MAX_BYTES {
                    warn!("ICS feed přes {MAX_BYTES} B, uříznut");
                    break;
                }
                body.extend_from_slice(&bytes);
            }
            Err(_) => break,
        }
    }

    Ok(parse_ics(&body, hours))
}

/// Mezistav jedné VEVENT před vyhodnocením vazeb (RRULE × EXDATE ×
/// RECURRENCE-ID). Google ruší a přesouvá jednotlivé instance opakované
/// schůzky právě přes RECURRENCE-ID, takže bez dvou průchodů se zrušená
/// instance nedá poznat.
struct RawEvent {
    uid: String,
    summary: String,
    start: ParsedStamp,
    tz: Option<chrono_tz::Tz>,
    rrule: Option<String>,
    cancelled: bool,
    exdates: Vec<i64>,
    recurrence_id: Option<i64>,
}

fn parse_ics(body: &[u8], hours: f64) -> Vec<CalEvent> {
    let now = chrono::Utc::now().timestamp();
    let until = now + (hours * 3600.0) as i64;

    let reader = ical::IcalParser::new(BufReader::new(body));
    let mut raw: Vec<RawEvent> = Vec::new();
    let mut skipped_bad = 0usize;
    let mut skipped_allday = 0usize;

    for cal in reader {
        let Ok(cal) = cal else {
            skipped_bad += 1;
            continue;
        };
        // strop i na surové záznamy — feed s 50k historií nemá smysl
        // zpracovávat celý (okno je stejně max 7 dní)
        for ev in cal.events.iter().take(MAX_EVENTS * 20) {
            let mut e = RawEvent {
                uid: String::new(),
                summary: String::new(),
                start: ParsedStamp::Missing,
                tz: None,
                rrule: None,
                cancelled: false,
                exdates: Vec::new(),
                recurrence_id: None,
            };
            for p in &ev.properties {
                match p.name.as_str() {
                    "SUMMARY" => e.summary = p.value.clone().unwrap_or_default(),
                    "UID" => e.uid = p.value.clone().unwrap_or_default(),
                    "RRULE" => e.rrule = p.value.clone(),
                    "STATUS" => {
                        e.cancelled = p
                            .value
                            .as_deref()
                            .is_some_and(|v| v.eq_ignore_ascii_case("CANCELLED"))
                    }
                    "DTSTART" => {
                        e.tz = tz_of(p);
                        e.start = parse_stamp(p);
                    }
                    // EXDATE smí být víckrát a hodnota smí být seznam
                    // oddělený čárkami
                    "EXDATE" => {
                        if let Some(v) = p.value.as_deref() {
                            let tz = tz_of(p);
                            for part in v.split(',') {
                                if let ParsedStamp::Timed(ts) = parse_value(part.trim(), tz) {
                                    e.exdates.push(ts);
                                }
                            }
                        }
                    }
                    "RECURRENCE-ID" => {
                        if let Some(v) = p.value.as_deref() {
                            if let ParsedStamp::Timed(ts) = parse_value(v.trim(), tz_of(p)) {
                                e.recurrence_id = Some(ts);
                            }
                        }
                    }
                    _ => {}
                }
            }
            raw.push(e);
        }
    }

    // 1. průchod: overridy instancí (RECURRENCE-ID). Zrušený override
    // instanci maže; přesunutý ji maže na původním čase a přidá sebe.
    use std::collections::{HashMap, HashSet};
    let mut overridden: HashMap<String, HashSet<i64>> = HashMap::new();
    for e in raw.iter().filter(|e| e.recurrence_id.is_some()) {
        overridden
            .entry(e.uid.clone())
            .or_default()
            .insert(e.recurrence_id.unwrap_or_default());
    }

    let mut out: Vec<CalEvent> = Vec::new();
    let push = |out: &mut Vec<CalEvent>, uid: &str, summary: &str, start: i64, occ: i64| {
        let iid = format!("ics:{}:{occ}", sanitize_title(uid));
        out.push(CalEvent {
            stable_id: iid.clone(),
            occurrence: occ,
            id: iid,
            title: sanitize_title(&unescape_ics(summary)),
            start,
            calendar_id: "ics".to_string(),
        });
    };

    // 2. průchod: samotné události
    'events: for e in &raw {
        if out.len() >= MAX_EVENTS {
            break;
        }
        // Zrušeno = zrušeno. Kontrola MUSÍ předcházet RRULE větvi —
        // dřív tudy zrušená opakovaná schůzka proklouzla a letěla dál.
        if e.cancelled {
            continue;
        }

        // Override s RECURRENCE-ID = přesunutá/upravená instance;
        // chová se jako jednorázová událost na svém novém čase.
        if e.recurrence_id.is_some() {
            if let ParsedStamp::Timed(start) = e.start {
                if start >= now && start <= until {
                    push(&mut out, &e.uid, &e.summary, start, start);
                }
            }
            continue;
        }

        match (&e.rrule, e.start) {
            (Some(rule), ParsedStamp::Timed(base)) => {
                let instances = expand_rrule(rule, base, e.tz, now, until);
                let cancelled_at = overridden.get(&e.uid);
                for inst in instances {
                    if e.exdates.contains(&inst) {
                        continue; // EXDATE = vyškrtnutá instance
                    }
                    if cancelled_at.is_some_and(|s| s.contains(&inst)) {
                        continue; // instanci nahrazuje/ruší override
                    }
                    push(&mut out, &e.uid, &e.summary, inst, inst);
                    if out.len() >= MAX_EVENTS {
                        break 'events;
                    }
                }
            }
            (None, ParsedStamp::Timed(start)) => {
                if start >= now && start <= until {
                    push(&mut out, &e.uid, &e.summary, start, start);
                }
            }
            // Celodenní schůzky nelétají — EventKit větev je filtruje
            // taky (narozeniny a svátky nejsou schůzky). Dřív tu vznikal
            // přelet v 9:00, nekonzistentně s druhým zdrojem.
            (_, ParsedStamp::AllDay) => skipped_allday += 1,
            (_, ParsedStamp::Missing) => skipped_bad += 1,
        }
    }

    if skipped_allday > 0 {
        info!("ICS: {skipped_allday} celodenních přeskočeno (nelétají)");
    }
    if skipped_bad > 0 {
        info!("ICS: {skipped_bad} vadných záznamů přeskočeno");
    }
    out.sort_by_key(|e| e.start);
    out
}

/// Rozbalí RRULE na konkrétní časy v okně ⟨from, to⟩. Expanduje v původní
/// časové zóně události — týdenní schůzka v 9:00 pražského času musí
/// zůstat v 9:00 i po přechodu na letní čas (v UTC se posune, lokálně ne).
/// Strop 100 instancí; při nesrozumitelném pravidle vrací prázdno.
fn expand_rrule(
    rule: &str,
    dtstart: i64,
    tz: Option<chrono_tz::Tz>,
    from: i64,
    to: i64,
) -> Vec<i64> {
    use chrono::TimeZone;
    let Some(start_utc) = chrono::Utc.timestamp_opt(dtstart, 0).single() else {
        return Vec::new();
    };
    // rrule crate čeká celý blok DTSTART + RRULE; TZID umí sám (nese
    // chrono-tz), stačí mu ho předat ve správném tvaru
    let spec = match tz {
        Some(tz) => format!(
            "DTSTART;TZID={}:{}\nRRULE:{}",
            tz.name(),
            start_utc.with_timezone(&tz).format("%Y%m%dT%H%M%S"),
            rule.trim()
        ),
        None => format!(
            "DTSTART:{}\nRRULE:{}",
            start_utc.format("%Y%m%dT%H%M%SZ"),
            rule.trim()
        ),
    };
    let Ok(rrule_set) = spec.parse::<rrule::RRuleSet>() else {
        return Vec::new();
    };
    let result = rrule_set.all(100);
    result
        .dates
        .into_iter()
        .map(|d| d.timestamp())
        .filter(|ts| *ts >= from && *ts <= to)
        .collect()
}

/// Výsledek čtení časové property: konkrétní okamžik, celodenní datum,
/// nebo nečitelné.
#[derive(Clone, Copy, PartialEq)]
enum ParsedStamp {
    Timed(i64),
    AllDay,
    Missing,
}

/// TZID parametr property → chrono-tz zóna. Neznámé jméno (Windows tvary
/// jako „Central Europe Standard Time") → None a hodnota se čte jako
/// lokální čas počítače; to je nejmenší škoda, jakou umíme udělat.
fn tz_of(p: &ical::property::Property) -> Option<chrono_tz::Tz> {
    let params = p.params.as_ref()?;
    let tzid = params
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("TZID"))
        .and_then(|(_, vs)| vs.first())?;
    tzid.trim().parse::<chrono_tz::Tz>().ok()
}

fn parse_stamp(p: &ical::property::Property) -> ParsedStamp {
    // VALUE=DATE = celodenní i kdyby hodnota vypadala jinak
    let is_date = p.params.as_ref().is_some_and(|ps| {
        ps.iter().any(|(k, vs)| {
            k.eq_ignore_ascii_case("VALUE") && vs.iter().any(|v| v.eq_ignore_ascii_case("DATE"))
        })
    });
    let Some(v) = p.value.as_deref() else {
        return ParsedStamp::Missing;
    };
    if is_date {
        return ParsedStamp::AllDay;
    }
    parse_value(v.trim(), tz_of(p))
}

/// Jedna hodnota data/času podle RFC 5545:
/// `…Z` = UTC · s TZID = čas v té zóně · bez obojího = lokální čas
/// počítače (floating) · `YYYYMMDD` = celodenní.
fn parse_value(v: &str, tz: Option<chrono_tz::Tz>) -> ParsedStamp {
    use chrono::TimeZone;
    if v.len() == 8 && v.chars().all(|c| c.is_ascii_digit()) {
        return ParsedStamp::AllDay;
    }
    if let Some(stripped) = v.strip_suffix('Z') {
        return match chrono::NaiveDateTime::parse_from_str(stripped, "%Y%m%dT%H%M%S") {
            Ok(dt) => ParsedStamp::Timed(dt.and_utc().timestamp()),
            Err(_) => ParsedStamp::Missing,
        };
    }
    let Ok(dt) = chrono::NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%S") else {
        return ParsedStamp::Missing;
    };
    // DST hrany: jarní neexistující čas posunout o hodinu vpřed (tak to
    // dělá i Kalendář), podzimní dvojznačný vzít dřívější výskyt.
    let resolve = |lr: chrono::LocalResult<chrono::DateTime<chrono_tz::Tz>>,
                   tz: chrono_tz::Tz,
                   dt: chrono::NaiveDateTime| {
        match lr {
            chrono::LocalResult::Single(t) => Some(t.timestamp()),
            chrono::LocalResult::Ambiguous(a, _) => Some(a.timestamp()),
            chrono::LocalResult::None => tz
                .from_local_datetime(&(dt + chrono::Duration::hours(1)))
                .earliest()
                .map(|t| t.timestamp()),
        }
    };
    match tz {
        Some(tz) => match resolve(tz.from_local_datetime(&dt), tz, dt) {
            Some(ts) => ParsedStamp::Timed(ts),
            None => ParsedStamp::Missing,
        },
        None => match dt.and_local_timezone(chrono::Local) {
            chrono::LocalResult::Single(t) => ParsedStamp::Timed(t.timestamp()),
            chrono::LocalResult::Ambiguous(a, _) => ParsedStamp::Timed(a.timestamp()),
            chrono::LocalResult::None => ParsedStamp::Missing,
        },
    }
}

/// ICS escapování: \, \; \n → skutečné znaky (a výsledek se pak znovu
/// sanitizuje v sanitize_title).
fn unescape_ics(s: &str) -> String {
    s.replace("\\n", " ")
        .replace("\\N", " ")
        .replace("\\,", ",")
        .replace("\\;", ";")
        .replace("\\\\", "\\")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ics_with_summary(summary: &str) -> Vec<u8> {
        let start = chrono::Utc::now() + chrono::Duration::hours(2);
        format!(
            "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:t1\r\nSUMMARY:{summary}\r\nDTSTART:{}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
            start.format("%Y%m%dT%H%M%SZ")
        )
        .into_bytes()
    }

    #[test]
    fn xss_v_summary_zustane_textem() {
        let evs = parse_ics(&ics_with_summary("<script>alert(1)</script>"), 24.0);
        assert_eq!(evs.len(), 1);
        // parser nic neinterpretuje; text jde dál jako data (React ho
        // renderuje jako text node)
        assert!(evs[0].title.contains("<script>"));
    }

    #[test]
    fn crlf_injection_je_odstranena() {
        let evs = parse_ics(&ics_with_summary("A\\nBEGIN:VEVENT zlo"), 24.0);
        assert_eq!(evs.len(), 1);
        assert!(!evs[0].title.contains('\n'));
        assert!(!evs[0].title.contains('\r'));
    }

    #[test]
    fn bidi_override_je_odstranen() {
        let evs = parse_ics(&ics_with_summary("Schůzka \u{202E}elzo"), 24.0);
        assert_eq!(evs.len(), 1);
        assert!(!evs[0].title.contains('\u{202E}'));
    }

    #[test]
    fn binarni_smeti_neshodi_parser() {
        let evs = parse_ics(&[0u8, 159, 146, 150, 13, 10, 66, 67], 24.0);
        assert!(evs.is_empty());
    }

    #[test]
    fn html_misto_ics_vrati_prazdno() {
        let evs = parse_ics(b"<html><body>login</body></html>", 24.0);
        assert!(evs.is_empty());
    }

    #[test]
    fn cap_500_udalosti() {
        let start = chrono::Utc::now() + chrono::Duration::hours(2);
        let mut body = String::from("BEGIN:VCALENDAR\r\n");
        for i in 0..1000 {
            body.push_str(&format!(
                "BEGIN:VEVENT\r\nUID:u{i}\r\nSUMMARY:E{i}\r\nDTSTART:{}\r\nEND:VEVENT\r\n",
                start.format("%Y%m%dT%H%M%SZ")
            ));
        }
        body.push_str("END:VCALENDAR\r\n");
        let evs = parse_ics(body.as_bytes(), 24.0);
        assert!(evs.len() <= MAX_EVENTS);
    }

    #[test]
    fn validace_url_odmitne_spatne() {
        // syntaktická vrstva — schválně bez DNS, ať testy běží offline
        assert!(validate_url_syntax("http://example.com/cal.ics").is_err());
        assert!(validate_url_syntax("https://localhost/cal.ics").is_err());
        assert!(validate_url_syntax("https://127.0.0.1/cal.ics").is_err());
        assert!(validate_url_syntax("https://[::1]/cal.ics").is_err());
        assert!(validate_url_syntax("https://intranet.local/cal.ics").is_err());
        assert!(validate_url_syntax("https://example.com:8080/cal.ics").is_err());
        assert!(validate_url_syntax("ftp://example.com/cal.ics").is_err());
        assert!(validate_url_syntax("https://calendar.google.com/calendar/ical/x/basic.ics").is_ok());
        assert!(validate_url_syntax("https://example.com:8443/cal.ics").is_ok());
    }

    #[test]
    fn zakazane_ip_pokryvaji_vsechny_rozsahy() {
        use std::net::IpAddr;
        let zakazane = [
            "127.0.0.1", "10.1.2.3", "172.16.0.1", "192.168.1.1",
            "169.254.1.1", "100.64.0.1", "100.127.255.255", "0.0.0.0",
            "::1", "fe80::1", "fc00::1", "fd12::1", "::ffff:10.0.0.1",
        ];
        for ip in zakazane {
            assert!(
                is_forbidden_ip(&ip.parse::<IpAddr>().unwrap()),
                "{ip} musí být zakázaná"
            );
        }
        let povolene = ["142.250.180.14", "2a00:1450:4014:80c::200e", "100.128.0.1"];
        for ip in povolene {
            assert!(
                !is_forbidden_ip(&ip.parse::<IpAddr>().unwrap()),
                "{ip} je veřejná a má projít"
            );
        }
    }

    #[test]
    fn rrule_se_rozbali_do_okna() {
        // denní opakování začínající před hodinou → v okně 24 h musí
        // vzniknout aspoň jedna instance (zítřejší výskyt)
        let start = chrono::Utc::now() - chrono::Duration::hours(1);
        let body = format!(
            "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:r1\r\nSUMMARY:Standup\r\nRRULE:FREQ=DAILY\r\nDTSTART:{}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
            start.format("%Y%m%dT%H%M%SZ")
        );
        let evs = parse_ics(body.as_bytes(), 24.0);
        assert!(!evs.is_empty(), "opakovaná událost se má rozbalit");
        assert!(evs.iter().all(|e| e.title == "Standup"));
    }

    fn vevent(body: &str) -> Vec<u8> {
        format!("BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\n{body}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n")
            .into_bytes()
    }

    fn za_hodin(h: i64) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now() + chrono::Duration::hours(h)
    }

    #[test]
    fn zrusena_jednorazova_neleti() {
        let b = vevent(&format!(
            "UID:c1\r\nSUMMARY:Zrušeno\r\nSTATUS:CANCELLED\r\nDTSTART:{}",
            za_hodin(2).format("%Y%m%dT%H%M%SZ")
        ));
        assert!(parse_ics(&b, 24.0).is_empty());
    }

    #[test]
    fn zrusena_opakovana_neleti() {
        // regrese Codex P0.2: CANCELLED musí platit i pro RRULE větev
        let b = vevent(&format!(
            "UID:c2\r\nSUMMARY:Zrušený standup\r\nSTATUS:CANCELLED\r\nRRULE:FREQ=DAILY\r\nDTSTART:{}",
            za_hodin(-1).format("%Y%m%dT%H%M%SZ")
        ));
        assert!(
            parse_ics(&b, 24.0).is_empty(),
            "zrušená opakovaná událost nesmí vytvořit přelet"
        );
    }

    #[test]
    fn zrusena_opakovana_neleti_pri_jinem_poradi_properties() {
        // STATUS až za RRULE — pořadí properties nesmí hrát roli
        let b = vevent(&format!(
            "UID:c3\r\nRRULE:FREQ=DAILY\r\nDTSTART:{}\r\nSUMMARY:X\r\nSTATUS:CANCELLED",
            za_hodin(-1).format("%Y%m%dT%H%M%SZ")
        ));
        assert!(parse_ics(&b, 24.0).is_empty());
    }

    #[test]
    fn exdate_vyskrtne_instanci() {
        let base = za_hodin(-23); // denní opakování, dnešní instance za ~1 h
        let dnesni = base + chrono::Duration::days(1);
        let b = vevent(&format!(
            "UID:x1\r\nSUMMARY:Standup\r\nRRULE:FREQ=DAILY\r\nDTSTART:{}\r\nEXDATE:{}",
            base.format("%Y%m%dT%H%M%SZ"),
            dnesni.format("%Y%m%dT%H%M%SZ")
        ));
        let evs = parse_ics(&b, 24.0);
        assert!(
            !evs.iter().any(|e| e.start == dnesni.timestamp()),
            "EXDATE instance nesmí letět"
        );
    }

    #[test]
    fn zrusena_instance_pres_recurrence_id_neleti() {
        // přesně takhle ruší jeden výskyt Google: master s RRULE +
        // samostatný VEVENT se stejným UID, RECURRENCE-ID a CANCELLED
        let base = za_hodin(-23);
        let dnesni = base + chrono::Duration::days(1);
        let body = format!(
            "BEGIN:VCALENDAR\r\n\
             BEGIN:VEVENT\r\nUID:g1\r\nSUMMARY:Sync\r\nRRULE:FREQ=DAILY\r\nDTSTART:{}\r\nEND:VEVENT\r\n\
             BEGIN:VEVENT\r\nUID:g1\r\nSUMMARY:Sync\r\nSTATUS:CANCELLED\r\nRECURRENCE-ID:{}\r\nDTSTART:{}\r\nEND:VEVENT\r\n\
             END:VCALENDAR\r\n",
            base.format("%Y%m%dT%H%M%SZ"),
            dnesni.format("%Y%m%dT%H%M%SZ"),
            dnesni.format("%Y%m%dT%H%M%SZ"),
        );
        let evs = parse_ics(body.as_bytes(), 24.0);
        assert!(
            !evs.iter().any(|e| e.start == dnesni.timestamp()),
            "instance zrušená přes RECURRENCE-ID nesmí letět"
        );
    }

    #[test]
    fn presunuta_instance_leti_na_novem_case() {
        let base = za_hodin(-23);
        let puvodni = base + chrono::Duration::days(1);
        let novy = puvodni + chrono::Duration::hours(3);
        let body = format!(
            "BEGIN:VCALENDAR\r\n\
             BEGIN:VEVENT\r\nUID:m1\r\nSUMMARY:Sync\r\nRRULE:FREQ=DAILY\r\nDTSTART:{}\r\nEND:VEVENT\r\n\
             BEGIN:VEVENT\r\nUID:m1\r\nSUMMARY:Sync (posunuto)\r\nRECURRENCE-ID:{}\r\nDTSTART:{}\r\nEND:VEVENT\r\n\
             END:VCALENDAR\r\n",
            base.format("%Y%m%dT%H%M%SZ"),
            puvodni.format("%Y%m%dT%H%M%SZ"),
            novy.format("%Y%m%dT%H%M%SZ"),
        );
        let evs = parse_ics(body.as_bytes(), 24.0);
        assert!(!evs.iter().any(|e| e.start == puvodni.timestamp()), "původní čas nesmí letět");
        assert!(evs.iter().any(|e| e.start == novy.timestamp()), "nový čas letět musí");
    }

    #[test]
    fn tzid_praha_se_prevede_spravne() {
        use chrono::TimeZone;
        let tz: chrono_tz::Tz = "Europe/Prague".parse().unwrap();
        let mistni = (chrono::Utc::now() + chrono::Duration::hours(3)).with_timezone(&tz);
        let b = vevent(&format!(
            "UID:tz1\r\nSUMMARY:Praha\r\nDTSTART;TZID=Europe/Prague:{}",
            mistni.format("%Y%m%dT%H%M%S")
        ));
        let evs = parse_ics(&b, 24.0);
        assert_eq!(evs.len(), 1);
        // převod zpět musí dát tentýž okamžik
        let zpet = tz.timestamp_opt(evs[0].start, 0).single().unwrap();
        assert_eq!(zpet.format("%H%M").to_string(), mistni.format("%H%M").to_string());
    }

    #[test]
    fn tzid_new_york_neni_lokalni_cas() {
        // 3 h dopředu v NY čase — kdyby se TZID ignoroval a četl jako
        // pražský lokální čas, výsledek by se lišil o hodiny
        let tz: chrono_tz::Tz = "America/New_York".parse().unwrap();
        let ny = (chrono::Utc::now() + chrono::Duration::hours(3)).with_timezone(&tz);
        let b = vevent(&format!(
            "UID:tz2\r\nSUMMARY:NY\r\nDTSTART;TZID=America/New_York:{}",
            ny.format("%Y%m%dT%H%M%S")
        ));
        let evs = parse_ics(&b, 24.0);
        assert_eq!(evs.len(), 1);
        let ocekavany = ny.timestamp();
        assert!((evs[0].start - ocekavany).abs() < 60, "TZID se musí respektovat");
    }

    #[test]
    fn neznamy_tzid_spadne_na_lokalni_cas() {
        let mistni = chrono::Local::now() + chrono::Duration::hours(3);
        let b = vevent(&format!(
            "UID:tz3\r\nSUMMARY:Windows TZ\r\nDTSTART;TZID=Central Europe Standard Time:{}",
            mistni.format("%Y%m%dT%H%M%S")
        ));
        let evs = parse_ics(&b, 24.0);
        assert_eq!(evs.len(), 1, "neznámý TZID nesmí událost zahodit");
        assert!((evs[0].start - mistni.timestamp()).abs() < 60);
    }

    #[test]
    fn celodenni_udalost_neleti() {
        let zitra = (chrono::Utc::now() + chrono::Duration::days(1)).format("%Y%m%d");
        let b = vevent(&format!("UID:ad1\r\nSUMMARY:Svátek\r\nDTSTART;VALUE=DATE:{zitra}"));
        assert!(
            parse_ics(&b, 48.0).is_empty(),
            "celodenní (svátky, narozeniny) nelétají — konzistence s EventKitem"
        );
    }

    #[test]
    fn rrule_bomba_je_omezena() {
        // FREQ=SECONDLY s obřím COUNT nesmí zahltit frontu
        let start = chrono::Utc::now() + chrono::Duration::minutes(1);
        let body = format!(
            "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:bomb\r\nSUMMARY:Bomba\r\nRRULE:FREQ=SECONDLY;COUNT=999999999\r\nDTSTART:{}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
            start.format("%Y%m%dT%H%M%SZ")
        );
        let t0 = std::time::Instant::now();
        let evs = parse_ics(body.as_bytes(), 24.0);
        assert!(evs.len() <= 100, "strop instancí musí držet");
        assert!(t0.elapsed().as_secs() < 3, "expanze nesmí trvat věčnost");
    }

}
