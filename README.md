# Ptáček 🐦

**Schůzky, které přeletí obrazovku.** Pár minut před schůzkou přeletí přes monitor maskot s upozorněním — přes všechna okna, i přes fullscreen prezentaci. Nejde přehlédnout a za pár vteřin sám zmizí.

Vyrobil [Transformuj.ai](https://transformuj.ai) · macOS 12+ · zdarma · MIT

🇬🇧 [README in English](README.en.md)

![Nastavení Ptáčka](docs/nastaveni.png)

## Co umí

- **10 maskotů**: oranžový ptáček, papírová vlaštovka, letadélko s transparentem, kocour, dron s balíčkem, robot poslíček, formule, balón, hejno ptáků skládající „TEĎ", pixel-art pták
- **Zdroje schůzek**: Apple Kalendář (EventKit, včetně Google/Outlook účtů připojených v systému) nebo tajná iCal adresa Google kalendáře (bez připojování účtu, uložená v Klíčence)
- **Hover**: najetí myší maskota zastaví, ukáže detail schůzky a „Odložit o 5 min"; kliky mimo maskota propadají skrz
- **Nastavení**: minuty předem, výběr maskota a kalendářů, rychlost přeletu, zvuk, ztlumení na hodinu/den, autostart
- **Dva monitory**: maskot letí přes displej, na kterém je právě kurzor
- **Reduce Motion**: se systémovým „Omezit pohyb" se místo přeletu ukáže tichá karta v rohu
- **Odinstalace**: sekce v Nastavení uklidí settings, LaunchAgent i Klíčenku; zbyde přetáhnout .app do koše
- **Klid**: overlay okno existuje jen během přeletu — mimo něj 0 % CPU

## Soukromí

Žádné servery, žádná telemetrie, žádné analytiky. Jediné síťové spojení je volitelná iCal adresa, kterou si sám nastavíš (HTTPS only, limity velikosti, validace). Kalendářová data neopouští počítač. Ověřitelné v kódu — proto je otevřený.

## Instalace

1. Stáhni DMG z Releases a přetáhni Ptáčka do Aplikací.
2. První spuštění: macOS ukáže varování (appka není podepsaná Apple certifikátem — nešíříme přes Apple). Otevři **Nastavení systému → Soukromí a zabezpečení → Přesto otevřít**.
3. Ptáček sedí v menu baru. Dej „Vyzkoušet teď" a povol přístup ke kalendáři.

## Vývoj

```bash
npm install
npx tauri dev     # vývoj
npx tauri build   # produkční DMG
cargo test --manifest-path src-tauri/Cargo.toml   # testy (vč. bezpečnostních ICS fixtures)
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets
```

Stack: Tauri v1 + React + TypeScript, EventKit přes objc2, maskoti = čisté CSS/SVG animace. Bezpečnostní architektura: minimální Tauri allowlist, CSP bez sítě pro webview, texty událostí výhradně jako text nodes, sanitizace titulků (C0/C1, bidi-override), ICS parser s limity.

## Podpora

Ptáček je dárek komunitě, ne produkt s podporou. Chybu nahlásit můžeš (issues jsou otevřené), opravu ani novou funkci ale neslibujeme. Pull requesty vítáme.

## Licence a záruka

MIT. Aplikace je poskytována „tak jak je", bez jakékoli záruky; za škody vzniklé používáním neneseme odpovědnost. Používáním s tím souhlasíš.

---

*Používáš Ptáčka? Máš 10% slevu na služby [Transformuj.ai](https://transformuj.ai) a vybraných partnerů — ozvi se a zmiň Ptáčka.*
