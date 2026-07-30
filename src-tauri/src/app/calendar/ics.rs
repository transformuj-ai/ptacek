//! ICS fallback: tajná iCal URL (Google/Outlook ji generují) → čtení
//! událostí bez OAuth a bez připojování účtu do macOS.
//!
//! Bezpečnostní mantinely (závazné, viz plán):
//! - URL jen HTTPS, ne privátní IP/localhost, port 443/8443
//! - URL je tajemství (nese token) → macOS Keychain, nikdy settings.json
//! - fetch: timeout 15 s / connect 5 s, redirecty max 3 a jen https,
//!   streamovaný limit 5 MB po dekompresi
//! - parser: tolerantní (vadný VEVENT se přeskočí), jen SUMMARY/DTSTART,
//!   max 500 událostí; RRULE se v v1 nerozbaluje (zaloguje se počet)
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

/// Ověří, že doména neresolvuje na zakázanou IP (volá se těsně před
/// fetchem — DNS se mezi uložením a stažením mohlo změnit).
fn resolves_to_public_ip(host: &str, port: u16) -> bool {
    use std::net::ToSocketAddrs;
    match (host, port).to_socket_addrs() {
        Ok(addrs) => {
            let addrs: Vec<_> = addrs.collect();
            !addrs.is_empty() && addrs.iter().all(|a| !is_forbidden_ip(&a.ip()))
        }
        Err(_) => false,
    }
}

pub fn validate_url(raw: &str) -> Result<(), String> {
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
    if !resolves_to_public_ip(&host, port) {
        return Err("Adresa směřuje do lokální sítě, to není povolené".into());
    }
    Ok(())
}

// --- Fetch + parse ---

pub async fn fetch_events(hours: f64) -> Vec<CalEvent> {
    let Some(url) = get_url() else {
        return Vec::new();
    };
    // fail-closed: hodnota z Klíčenky se validuje znovu (mohla ji tam
    // zapsat jiná cesta než naše UI)
    if validate_url(&url).is_err() {
        warn!("ICS: uložená adresa neprošla validací, přeskakuji");
        return Vec::new();
    }

    let client = match reqwest::Client::builder()
        .https_only(true)
        // Redirect musí projít STEJNOU validací jako zadaná URL —
        // jinak by feed mohl přesměrovat na 127.0.0.1 nebo interní síť
        // a obejít všechny zákazy (SSRF).
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 3 {
                return attempt.error("příliš mnoho přesměrování");
            }
            match validate_url(attempt.url().as_str()) {
                Ok(()) => attempt.follow(),
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
            return Vec::new();
        }
    };

    let resp = match client.get(url).send().await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            warn!("ICS feed vrátil status {}", r.status());
            return Vec::new();
        }
        Err(e) => {
            // e může obsahovat URL s tokenem → logovat jen druh chyby
            warn!(
                "ICS fetch selhal ({})",
                if e.is_timeout() { "timeout" } else if e.is_connect() { "connect" } else { "chyba" }
            );
            return Vec::new();
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

    parse_ics(&body, hours)
}

fn parse_ics(body: &[u8], hours: f64) -> Vec<CalEvent> {
    let now = chrono::Utc::now().timestamp();
    let until = now + (hours * 3600.0) as i64;

    let reader = ical::IcalParser::new(BufReader::new(body));
    let mut out: Vec<CalEvent> = Vec::new();
    let mut skipped_rrule = 0usize;
    let mut skipped_bad = 0usize;

    'outer: for cal in reader {
        let Ok(cal) = cal else {
            skipped_bad += 1;
            continue;
        };
        for ev in cal.events {
            if out.len() >= MAX_EVENTS {
                break 'outer;
            }
            let mut summary = String::new();
            let mut dtstart: Option<i64> = None;
            let mut uid = String::new();
            let mut rrule_line: Option<String> = None;
            let mut status_cancelled = false;

            for p in &ev.properties {
                match p.name.as_str() {
                    "SUMMARY" => summary = p.value.clone().unwrap_or_default(),
                    "UID" => uid = p.value.clone().unwrap_or_default(),
                    "RRULE" => rrule_line = p.value.clone(),
                    "STATUS" => {
                        status_cancelled =
                            p.value.as_deref().is_some_and(|v| v.eq_ignore_ascii_case("CANCELLED"))
                    }
                    "DTSTART" => dtstart = parse_dtstart(p),
                    _ => {}
                }
            }

            // Opakovaná událost: rozbalit instance spadající do okna.
            // Strop 100 instancí chrání proti RRULE bombě
            // (FREQ=SECONDLY;COUNT=999999999).
            if let Some(rule) = rrule_line.as_deref() {
                let Some(base) = dtstart else {
                    skipped_bad += 1;
                    continue;
                };
                let instances = expand_rrule(rule, base, now, until);
                if instances.is_empty() {
                    skipped_rrule += 1;
                }
                for inst_start in instances {
                    let iid = format!("ics:{}:{inst_start}", sanitize_title(&uid));
                    out.push(CalEvent {
                        stable_id: iid.clone(),
                        occurrence: inst_start,
                        id: iid,
                        title: sanitize_title(&unescape_ics(&summary)),
                        start: inst_start,
                        calendar_id: "ics".to_string(),
                    });
                    if out.len() >= MAX_EVENTS {
                        break 'outer;
                    }
                }
                continue;
            }
            if status_cancelled {
                continue;
            }
            let Some(start) = dtstart else {
                skipped_bad += 1;
                continue;
            };
            if start < now || start > until {
                continue;
            }

            let iid = format!("ics:{}", sanitize_title(&uid));
            out.push(CalEvent {
                stable_id: iid.clone(),
                occurrence: start,
                id: iid,
                title: sanitize_title(&unescape_ics(&summary)),
                start,
                calendar_id: "ics".to_string(),
            });
        }
    }

    if skipped_rrule > 0 {
        info!("ICS: {skipped_rrule} opakovaných událostí přeskočeno (v1 limit)");
    }
    if skipped_bad > 0 {
        info!("ICS: {skipped_bad} vadných záznamů přeskočeno");
    }
    out.sort_by_key(|e| e.start);
    out
}

/// Rozbalí RRULE na konkrétní časy v okně ⟨from, to⟩.
/// Strop 100 instancí; při nesrozumitelném pravidle vrací prázdno.
fn expand_rrule(rule: &str, dtstart: i64, from: i64, to: i64) -> Vec<i64> {
    use chrono::TimeZone;
    let Some(start_dt) = chrono::Utc.timestamp_opt(dtstart, 0).single() else {
        return Vec::new();
    };
    // rrule crate čeká celý blok DTSTART + RRULE
    let spec = format!(
        "DTSTART:{}\nRRULE:{}",
        start_dt.format("%Y%m%dT%H%M%SZ"),
        rule.trim()
    );
    let Ok(rrule_set) = spec.parse::<rrule::RRuleSet>() else {
        return Vec::new();
    };
    let result = rrule_set.all(100);
    result.dates
        .into_iter()
        .map(|d| d.timestamp())
        .filter(|ts| *ts >= from && *ts <= to)
        .collect()
}

/// DTSTART: podporuje UTC tvar (…Z), lokální bez TZ a celodenní datum.
/// TZID parametry v1 čte jako lokální čas (Google posílá hlavně UTC).
fn parse_dtstart(p: &ical::property::Property) -> Option<i64> {
    let v = p.value.as_deref()?.trim();
    // celodenní: YYYYMMDD
    if v.len() == 8 && v.chars().all(|c| c.is_ascii_digit()) {
        let d = chrono::NaiveDate::parse_from_str(v, "%Y%m%d").ok()?;
        let dt = d.and_hms_opt(9, 0, 0)?; // celodenní → připomenout v 9:00
        return dt.and_local_timezone(chrono::Local).single().map(|t| t.timestamp());
    }
    // UTC: YYYYMMDDTHHMMSSZ
    if let Some(stripped) = v.strip_suffix('Z') {
        let dt = chrono::NaiveDateTime::parse_from_str(stripped, "%Y%m%dT%H%M%S").ok()?;
        return Some(dt.and_utc().timestamp());
    }
    // lokální: YYYYMMDDTHHMMSS
    let dt = chrono::NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%S").ok()?;
    dt.and_local_timezone(chrono::Local).single().map(|t| t.timestamp())
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
        assert!(validate_url("http://example.com/cal.ics").is_err());
        assert!(validate_url("https://localhost/cal.ics").is_err());
        assert!(validate_url("https://127.0.0.1/cal.ics").is_err());
        assert!(validate_url("https://[::1]/cal.ics").is_err());
        assert!(validate_url("https://example.com:8080/cal.ics").is_err());
        assert!(validate_url("ftp://example.com/cal.ics").is_err());
        assert!(validate_url("https://calendar.google.com/calendar/ical/x/basic.ics").is_ok());
        assert!(validate_url("https://example.com:8443/cal.ics").is_ok());
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
