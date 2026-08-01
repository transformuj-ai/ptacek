pub mod eventkit;
pub mod ics;
pub mod service;

use unicode_normalization_shim::nfc_shim;

/// Jedna sanitizační funkce pro tituly z VŠECH zdrojů (EventKit i budoucí
/// ICS): odstraní řídicí a bidi-override znaky, normalizuje a ořízne.
/// Texty jdou do UI výhradně jako text nodes, tohle je vrstva navíc.
pub fn sanitize_title(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| if c == '\r' || c == '\n' || c == '\t' { ' ' } else { c })
        .filter(|c| {
            let u = *c as u32;
            // C0/C1 řídicí znaky
            let control = u < 0x20 || (0x7F..=0x9F).contains(&u);
            // bidi override / isolate (spoofing textu)
            let bidi = (0x202A..=0x202E).contains(&u) || (0x2066..=0x2069).contains(&u);
            !(control || bidi)
        })
        .collect();

    let normalized = nfc_shim(cleaned.trim());

    // ořez na 120 znaků (chars ≈ grapheme pro naše účely; plný
    // unicode-segmentation přidáme s ICS parserem v PR9)
    normalized.chars().take(120).collect()
}

/// Zkrátí ukecané názvy z rezervačních systémů do čitelné podoby.
/// cal.com dělá „Intro konzultace | Transformuj.ai mezi Transformuj.ai
/// a Filip Oborník" — přeletem se to přečíst nedá. Výsledek:
/// „Intro konzultace · Filip Oborník". Když vzor nesedí, vrací původní
/// text beze změny (nikdy nehádáme).
pub fn tidy_title(raw: &str) -> String {
    let t = raw.split_whitespace().collect::<Vec<_>>().join(" ");

    for (between, and) in [(" mezi ", " a "), (" between ", " and ")] {
        let Some(bpos) = t.find(between) else { continue };
        let head = t[..bpos].trim();
        let tail_part = &t[bpos + between.len()..];
        let Some(apos) = tail_part.rfind(and) else { continue };
        let first_party = tail_part[..apos].trim();
        let other = tail_part[apos + and.len()..].trim();
        // Obě strany musí vypadat jako jméno osoby/firmy, jinak jde
        // nejspíš o běžnou větu („porada mezi odděleními o rozpočtu
        // a plánování kapacit") a do té nesaháme.
        if !looks_like_name(first_party) || !looks_like_name(other) {
            continue;
        }
        // z hlavičky vzít jen část před oddělovačem („Intro konzultace |
        // Transformuj.ai" → „Intro konzultace")
        let subject = head
            .split(['|', '–', '—'])
            .next()
            .unwrap_or(head)
            .trim()
            .trim_end_matches('-')
            .trim();
        let out = if subject.is_empty() {
            other.to_string()
        } else {
            format!("{subject} · {other}")
        };
        if out.chars().count() < t.chars().count() {
            return out;
        }
    }
    t
}

/// Vypadá řetězec jako jméno člověka nebo firmy? (1–4 slova, každé
/// začíná velkým písmenem nebo číslicí, rozumná délka)
fn looks_like_name(s: &str) -> bool {
    let words: Vec<&str> = s.split_whitespace().collect();
    if words.is_empty() || words.len() > 4 || s.chars().count() > 40 {
        return false;
    }
    words.iter().all(|w| {
        w.chars()
            .next()
            .is_some_and(|c| c.is_uppercase() || c.is_ascii_digit())
    })
}

/// Minimalistický NFC shim bez závislosti: pro MVP necháváme text tak,
/// jak přišel z EventKitu (NSString je už složený tvar). Plná NFC
/// normalizace přijde s crate unicode-normalization v PR9 (ICS).
mod unicode_normalization_shim {
    pub fn nfc_shim(s: &str) -> String {
        s.to_string()
    }
}

#[cfg(test)]
mod tidy_tests {
    use super::tidy_title;

    #[test]
    fn calcom_vzor_se_zkrati() {
        let t = tidy_title(
            "Intro konzultace | Transformuj.ai mezi Transformuj.ai a Filip Oborník",
        );
        assert_eq!(t, "Intro konzultace · Filip Oborník");
    }

    #[test]
    fn anglicky_vzor() {
        let t = tidy_title("Discovery call between Transformuj.ai and John Smith");
        assert_eq!(t, "Discovery call · John Smith");
    }

    #[test]
    fn bezny_nazev_zustane() {
        assert_eq!(tidy_title("Standup"), "Standup");
        assert_eq!(tidy_title("Oběd s Tomášem"), "Oběd s Tomášem");
    }

    #[test]
    fn nesmyslny_vzor_neposkodi() {
        // „mezi" bez rozumného protějšku → beze změny
        let long = "Porada mezi odděleními o rozpočtu a plánování kapacit na příští kvartál";
        assert_eq!(tidy_title(long), long);
    }
}
