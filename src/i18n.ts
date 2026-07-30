// Mini i18n bez závislostí. Jazyk žije v nastavení (settings.language),
// default čeština. Rust strana (tray) se přepíná commandem set_language.

export type Lang = "cs" | "en";

const STRINGS = {
  cs: {
    tagline: "Schůzky, které přeletí obrazovku · Vyrobeno v Transformuj.ai",
    tryFlyby: "Vyzkoušet přelet",
    secFlyby: "Přelet",
    textMode: "Co má maskot říkat",
    textModeTitle: "Název schůzky (doporučeno)",
    textModeFun: "Vtipnou hlášku",
    textModeHybrid: "Hlášku i název schůzky",
    textModeHint:
      "Hlášky jsou obecné, takže sedí na jakoukoli schůzku a nic o ní neprozradí — hodí se, když sdílíš obrazovku.",
    /** Obecné hlášky — nesmí tvrdit nic konkrétního o schůzce */
    funLines: [
      "Za chvíli tě někdo bude hledat",
      "Kalendář má na tebe nápad",
      "Tohle už začíná",
      "Někdo si na tebe udělal čas",
      "Čas dopsat větu",
      "Kafe teď, nebo nikdy",
      "Zkontroluj mikrofon",
      "Tvoje budoucí já ti děkuje",
      "Za chvíli se hraje",
      "Není to past, je to schůzka",
      "Poslední minuty klidu",
      "Přepni si mozek",
    ],
    /** Vysvětlení v tiché kartě, když má systém zapnuté Omezit pohyb */
    reducedWhy:
      "Máš v systému zapnuté „Omezit pohyb“, tak Ptáček nikam nelétá a jen tiše upozorní. Zpátky ho pustíš v Nastavení systému → Zpřístupnění → Zobrazení.",
    motionNotice:
      "Systém má zapnuté „Omezit pohyb“ (Nastavení systému → Zpřístupnění → Zobrazení). Ptáček to respektuje: místo přeletu ukáže v rohu tichou kartu se stejnými tlačítky. Vypnutím té volby se maskoti vrátí.",
    tabSettings: "Nastavení",
    tabGuide: "Návod",
    guideIntro:
      "Ptáček sedí v horní liště a pár minut před schůzkou pustí přes obrazovku maskota. Nejde přehlédnout, za pár vteřin sám zmizí a nic nikam neposílá.",
    guideStartTitle: "Než začneš",
    guideStart: [
      "Klikni na ptáčka v horní liště → Nastavení a povol přístup ke kalendáři. Bez toho Ptáček neví, kdy má letět.",
      "Nemáš schůzky v Apple Kalendáři? Připoj Google účet v Nastavení systému → Internetové účty (stačí zapnout Kalendáře), nebo použij tajnou iCal adresu — návod je v sekci níže.",
      "Zaškrtni kalendáře, které tě mají zajímat. Narozeniny a svátky necháváme vypnuté, aby ti maskot nelítal každý den.",
      "Vyber si maskota a rychlost, dej „Vyzkoušet přelet“ a je hotovo.",
    ],
    guideDailyTitle: "Každodenní používání",
    guideDaily: [
      "Najetí myší na letícího maskota ho zastaví a ukáže detail schůzky. Můžeš ji odložit o 5 minut nebo přelet zavřít.",
      "Kliknutí mimo maskota propadne do aplikace pod ním — Ptáček ti nikdy neblokuje práci.",
      "Potřebuješ klid? V liště je „Ztlumit na 1 hodinu“ nebo „Ztlumit do konce dne“. Kliknutím na tutéž položku ztlumení zrušíš.",
      "Maskot letí i přes prezentaci na celou obrazovku — právě proto vznikl.",
      "Máš dva monitory? Maskot přeletí přes ten, na kterém máš právě myš, ať ti neuteče na vedlejší obrazovku.",
    ],
    guideTroubleTitle: "Když něco nefunguje",
    guideTrouble: [
      "Maskot nelétá: zkontroluj v sekci Kalendáře, že má Ptáček přístup a že je zaškrtnutý ten správný kalendář.",
      "Schůzka přidaná před chvílí: Apple si data z Googlu tahá s malým zpožděním, Ptáček je kontroluje každých pět minut.",
      "Po aktualizaci appky se macOS znovu zeptá na kalendář — potvrď dialog a je to.",
      "Maskot nikam nelétá a místo něj naskočí karta v rohu: máš v systému zapnuté „Omezit pohyb“. Je to záměr, Ptáček to respektuje (Nastavení systému → Zpřístupnění → Zobrazení).",
      "Chceš Ptáčka pryč? Úplně dole v Nastavení je Odinstalace — uklidí po sobě a otevře ti složku Aplikace, kde ho hodíš do koše.",
      "Nic z toho nepomohlo? Napiš na jakub@transformuj.ai.",
    ],

    notifyBefore: "Upozornit před schůzkou",
    atStart: "v čase začátku",
    minBefore: (n: number) => `${n} minut${n === 1 ? "u" : "y"} předem`,
    mascot: "Maskot",
    randomMascot: "Náhodný (pokaždé jiný)",
    speed: "Rychlost přeletu",
    speedSlow: "Pomalu (ptáček letí ~12 s)",
    speedNormal: "Normálně (~8 s)",
    speedFast: "Rychle (~5 s)",
    secBehavior: "Chování",
    launchAtLogin: "Spouštět po přihlášení",
    sound: "Zvuk při přeletu",
    language: "Jazyk / Language",
    secMascots: "Maskoti",
    mascotsHint:
      "Rozklikni maskota, přečti si, co dělá, a pusť si ukázku přes celou obrazovku.",
    play: "Přehrát ukázku",
    secCalendars: "Kalendáře",
    calHint:
      "Maskot letí pro schůzky ze zaškrtnutých kalendářů. Máš schůzky v Googlu nebo Outlooku? Připoj účet v Nastavení systému → Internetové účty → Google (stačí zapnout Kalendáře) a do minuty se objeví i tady.",
    calNoAccess:
      "Ptáček zatím nemá přístup ke kalendáři. Bez něj neví, kdy ti má maskot přeletět — potvrď prosím systémový dialog, který se objeví po kliknutí.",
    calGrant: "Povolit přístup",
    calAsking: "Čekám na tvoje potvrzení…",
    calOkNone:
      "Hotovo, přístup je povolený. V příštích 24 hodinách zatím žádnou schůzku nevidím — až nějakou přidáš, maskot přiletí sám.",
    calOk: (cals: number, events: number) =>
      `Hotovo, přístup je povolený. Vidím ${cals} kalendářů a ${events} ${
        events === 1 ? "schůzku" : events < 5 ? "schůzky" : "schůzek"
      } v příštích 24 hodinách.`,
    calRefused:
      "Přístup nebyl udělen. Můžeš ho kdykoli zapnout v Nastavení systému → Soukromí a zabezpečení → Kalendáře, nebo použít adresu kalendáře níže.",
    calDenied:
      "Přístup ke kalendáři je zamítnutý. Povol ho v Nastavení systému → Soukromí a zabezpečení → Kalendáře.",
    secIcs: "Google kalendář bez připojování účtu",
    icsHint:
      "Nechceš Google účet přidávat do Macu? Google umí pro kalendář vytvořit soukromý odkaz — funguje jako klíč od jedněch dveří: kdo ho má, vidí jen kdy máš schůzky. Nedostane se do e-mailu a nemůže nic měnit.",
    icsSteps: [
      "Otevři calendar.google.com a přihlas se.",
      "Vpravo nahoře ozubené kolečko → Nastavení.",
      "Vlevo v „Nastavení mých kalendářů“ klikni na svůj kalendář.",
      "Sjeď na sekci „Integrovat kalendář“.",
      "Úplně dole zkopíruj „Tajná adresa ve formátu iCal“ (končí .ics).",
      "Vlož ji sem a dej Uložit — Ptáček rovnou napíše, kolik schůzek našel.",
    ],
    icsSafety:
      "Adresa se uloží do Klíčenky macOS, tedy tam, kde má Mac tvá hesla. Nikam se neposílá, Ptáček s ní jen jednou za pět minut sáhne na Google pro seznam schůzek. Kdykoli ji tady odebereš; kdyby unikla, stačí ji v Google Kalendáři obnovit a stará přestane platit.",
    secSigning: "Proč macOS varuje při prvním spuštění",
    signingText:
      "Ptáčka nešíříme přes App Store ani ho nepodepisujeme placeným certifikátem Applu — je to dárek komunitě, který chceme dát k dispozici bez poplatků a prostředníka. Prakticky to znamená dvě věci: při prvním spuštění je potřeba appku jednou povolit v Nastavení systému, a po každé aktualizaci se macOS znovu zeptá na přístup ke kalendáři (bere novou verzi jako novou aplikaci). Na oplátku je celý kód veřejný, takže si každý může ověřit, co appka dělá — nemá servery ani telemetrii a jediné spojení, které kdy naváže, je adresa kalendáře, kterou si sám zadáš. Potřebuješ podepsanou verzi do firmy? Ozvi se nám.",
    icsSave: "Uložit",
    icsChecking: "Ověřuji…",
    icsSet: "iCal adresa je nastavená",
    icsRemove: "Odebrat",
    icsOk: (n: number) => `Připojeno, ${n} událostí v příštích 7 dnech`,
    secAbout: "O aplikaci",
    whyTitle: "Proč Ptáček existuje?",
    whyText:
      "Protože notifikace macOS je zdvořilá, decentní a dokonale přehlédnutelná. Pták letící přes celý monitor přehlédnutelný není. Vyrobil ho tým Transformuj.ai AI-first vývojem za pár dní a pouští ho zdarma, protože nejlepší reklama na AI tým je věc, která funguje.",
    perk:
      "Používáš Ptáčka? Máš 10% slevu na služby Transformuj.ai a vybraných partnerů. Stačí se ozvat a zmínit Ptáčka.",
    disclaimer: (v: string) =>
      `Verze ${v} · MIT licence · Ptáček je dílem společnosti Appmine, s.r.o. (IČO 03080137) a jejího projektu Transformuj.ai — nápad, návrh i výroba. Při dalším šíření, úpravách nebo prezentování aplikace uváděj, že ji navrhl a vyrobil tým Transformuj.ai. Aplikace je poskytována „tak jak je", bez jakékoli záruky — Appmine, s.r.o. ani Transformuj.ai nenesou odpovědnost za funkčnost ani za škody vzniklé používáním, a to v maximálním rozsahu povoleném právem. Používáním aplikace s tím souhlasíš. Žádná data neopouští tvůj počítač — appka nemá servery, telemetrii ani analytiku.`,
    secPartner: "Partner",
    partnerName: "Filip Oborník",
    partnerRole: "AI a technologický konzultant · školení a mentoring",
    partnerQuote: "„Technologie a AI jsou jen nástroje k výsledku.“",
    partnerText:
      "Pomáhá firmám i jednotlivcům zapojit AI a vibe coding do praxe — workshopy, 1:1 mentoring a technické konzultace. U partnerských projektů zajišťuje AI a technickou expertízu od návrhu po nasazení. Náš externí partner pro AI školení a implementaci.",
    partnerTags: ["AI školení", "Vibe coding", "Mentoring", "Tech konzultace"],
    partnerLink: "YouTube: AI s rozumem →",
    photoAlt: "Jakub Liška — Co-founder Transformuj.ai",
    photoCaption: "Jakub Liška · Co-founder Transformuj.ai",
    linkWeb: "transformuj.ai →",
    linkLi: "LinkedIn: Jakub Liška →",
    secUninstall: "Odinstalace",
    uninstallText:
      "Přetažení do koše samo o sobě nechá na disku nastavení, spouštění po přihlášení a adresu kalendáře v Klíčence. Tímhle tlačítkem po sobě Ptáček všechno uklidí a otevře ti složku Aplikace, kde ho pak stačí hodit do koše. Tvůj kalendář ani nic jiného se nedotkne.",
    uninstallBtn: "Odinstalovat Ptáčka…",
    uninstallConfirm:
      "Ptáček smaže svoje nastavení, vypne spouštění po přihlášení, odebere adresu kalendáře z Klíčenky a ukončí se. Pokračovat?",
    uninstallYes: "Ano, uklidit a ukončit",
    uninstallCancel: "Zrušit",
  },
  en: {
    tagline: "Meetings that fly across your screen · Made by Transformuj.ai",
    tryFlyby: "Try a flyby",
    secFlyby: "Flyby",
    textMode: "What the mascot says",
    textModeTitle: "Meeting name (recommended)",
    textModeFun: "A witty line",
    textModeHybrid: "Line and meeting name",
    textModeHint:
      "The lines are generic, so they fit any meeting and reveal nothing about it — handy when you share your screen.",
    funLines: [
      "Someone is about to look for you",
      "Your calendar has plans",
      "This one is starting",
      "Someone booked time for you",
      "Time to finish that sentence",
      "Coffee now or never",
      "Check your microphone",
      "Your future self says thanks",
      "Showtime in a moment",
      "It's not a trap, it's a meeting",
      "Last quiet minutes",
      "Switch your brain over",
    ],
    reducedWhy:
      "Your Mac has “Reduce motion” turned on, so Ptáček stays put and just tells you quietly. Turn it off in System Settings → Accessibility → Display to get the mascots back.",
    motionNotice:
      "Your Mac has “Reduce motion” turned on (System Settings → Accessibility → Display). Ptáček respects it: instead of a flyby you get a quiet card in the corner with the same buttons. Turn that option off and the mascots come back.",
    tabSettings: "Settings",
    tabGuide: "Guide",
    guideIntro:
      "Ptáček lives in your menu bar and sends a mascot across the screen a few minutes before a meeting. Impossible to miss, gone in seconds, and nothing ever leaves your Mac.",
    guideStartTitle: "Before you start",
    guideStart: [
      "Click the bird in the menu bar → Settings and grant calendar access. Without it Ptáček cannot know when to fly.",
      "Meetings not in Apple Calendar? Connect your Google account in System Settings → Internet Accounts (just enable Calendars), or use the secret iCal address — see the section below.",
      "Tick the calendars you care about. Birthdays and holidays stay off so the mascot doesn't fly every day.",
      "Pick a mascot and speed, hit “Try a flyby”, and you're done.",
    ],
    guideDailyTitle: "Everyday use",
    guideDaily: [
      "Hover the flying mascot to freeze it and see the meeting details. You can snooze it 5 minutes or close it.",
      "Clicks outside the mascot pass through to the app below — Ptáček never blocks your work.",
      "Need quiet? The menu bar has “Mute for 1 hour” and “Mute until tomorrow”. Click again to unmute.",
      "The mascot flies over full-screen presentations too — that's exactly why it exists.",
      "Two monitors? The mascot flies across the one your mouse is on, so you never miss it on the other screen.",
    ],
    guideTroubleTitle: "If something doesn't work",
    guideTrouble: [
      "No mascot: check in the Calendars section that Ptáček has access and the right calendar is ticked.",
      "Meeting added a minute ago: Apple syncs from Google with a slight delay, and Ptáček checks every five minutes.",
      "After an app update macOS asks for calendar access again — just confirm the dialog.",
      "No mascot, just a card in the corner: you have “Reduce motion” turned on. That's intentional, Ptáček respects it (System Settings → Accessibility → Display).",
      "Want Ptáček gone? The Uninstall section at the very bottom of Settings cleans up after itself and opens your Applications folder so you can drop it in the Trash.",
      "Still stuck? Write to jakub@transformuj.ai.",
    ],

    notifyBefore: "Notify before meeting",
    atStart: "at start time",
    minBefore: (n: number) => `${n} minute${n === 1 ? "" : "s"} before`,
    mascot: "Mascot",
    randomMascot: "Random (different every time)",
    speed: "Flyby speed",
    speedSlow: "Slow (bird takes ~12 s)",
    speedNormal: "Normal (~8 s)",
    speedFast: "Fast (~5 s)",
    secBehavior: "Behavior",
    launchAtLogin: "Launch at login",
    sound: "Sound on flyby",
    language: "Language / Jazyk",
    secMascots: "Mascots",
    mascotsHint:
      "Expand a mascot to see what it does, then play a full-screen preview.",
    play: "Play preview",
    secCalendars: "Calendars",
    calHint:
      "The mascot flies for meetings from checked calendars. Keep your meetings in Google or Outlook? Connect the account in System Settings → Internet Accounts → Google (enable Calendars) and they show up here within a minute.",
    calNoAccess:
      "Ptáček has no calendar access yet. Without it, it cannot know when to send the mascot — please confirm the system dialog that appears after you click.",
    calGrant: "Grant access",
    calAsking: "Waiting for your confirmation…",
    calOkNone:
      "Done, access granted. No meetings in the next 24 hours yet — as soon as you add one, the mascot will fly on its own.",
    calOk: (cals: number, events: number) =>
      `Done, access granted. I can see ${cals} calendars and ${events} ${
        events === 1 ? "meeting" : "meetings"
      } in the next 24 hours.`,
    calRefused:
      "Access was not granted. You can enable it anytime in System Settings → Privacy & Security → Calendars, or use the calendar address below.",
    calDenied:
      "Calendar access is denied. Allow it in System Settings → Privacy & Security → Calendars.",
    secIcs: "Google Calendar without connecting an account",
    icsHint:
      "Don't want to add your Google account to the Mac? Google can create a private link for a calendar — it works like a key to one door: whoever has it can only see when you have meetings. No access to your email, no way to change anything.",
    icsSteps: [
      "Open calendar.google.com and sign in.",
      "Top right: gear icon → Settings.",
      "On the left under “Settings for my calendars” pick your calendar.",
      "Scroll to the “Integrate calendar” section.",
      "At the very bottom copy the “Secret address in iCal format” (ends with .ics).",
      "Paste it here and hit Save — Ptáček tells you right away how many meetings it found.",
    ],
    icsSafety:
      "The address goes into the macOS Keychain, the same vault that holds your passwords. It is never sent anywhere else; Ptáček only uses it every five minutes to ask Google for your schedule. You can remove it here anytime, and if it ever leaks, regenerate it in Google Calendar — the old one stops working.",
    secSigning: "Why macOS warns on first launch",
    signingText:
      "Ptáček is not distributed through the App Store and is not signed with a paid Apple certificate — it is a gift to the community and we want it available with no fees and no middleman. In practice this means two things: on first launch you have to allow the app once in System Settings, and after every update macOS asks for calendar access again (it treats the new version as a new app). In exchange, the whole source code is public so anyone can verify what the app does — no servers, no telemetry, and the only connection it ever makes is the calendar address you enter yourself. Need a signed build for your company? Get in touch.",
    icsSave: "Save",
    icsChecking: "Checking…",
    icsSet: "iCal address is set",
    icsRemove: "Remove",
    icsOk: (n: number) => `Connected, ${n} events in the next 7 days`,
    secAbout: "About",
    whyTitle: "Why does Ptáček exist?",
    whyText:
      "Because the macOS notification is polite, subtle, and perfectly missable. A bird flying across your entire monitor is not. The Transformuj.ai team built it in a few days of AI-first development and gives it away for free — the best ad for an AI team is a thing that works.",
    perk:
      "Using Ptáček? You get a 10% discount on Transformuj.ai services and selected partners. Just reach out and mention Ptáček.",
    disclaimer: (v: string) =>
      `Version ${v} · MIT license · Ptáček is a creation of Appmine, s.r.o. (Company ID 03080137) and its project Transformuj.ai — idea, design, and build. When redistributing, modifying, or presenting the app, credit the Transformuj.ai team as its designer and maker. The app is provided “as is”, without any warranty — Appmine, s.r.o. and Transformuj.ai accept no liability for functionality or damages arising from use, to the maximum extent permitted by law. By using the app you agree to this. No data ever leaves your computer — the app has no servers, telemetry, or analytics.`,
    secPartner: "Partner",
    partnerName: "Filip Oborník",
    partnerRole: "AI and technology consultant · training and mentoring",
    partnerQuote: "“Technology and AI are just tools to get a result.”",
    partnerText:
      "Helps companies and individuals put AI and vibe coding to work — workshops, 1:1 mentoring, and technical consulting. On partner projects he covers AI and technical expertise from design to deployment. Our external partner for AI training and implementation.",
    partnerTags: ["AI training", "Vibe coding", "Mentoring", "Tech consulting"],
    partnerLink: "YouTube: AI s rozumem →",
    photoAlt: "Jakub Liška — Co-founder of Transformuj.ai",
    photoCaption: "Jakub Liška · Co-founder, Transformuj.ai",
    linkWeb: "transformuj.ai →",
    linkLi: "LinkedIn: Jakub Liška →",
    secUninstall: "Uninstall",
    uninstallText:
      "Dragging the app to the Trash on its own leaves behind your settings, the launch-at-login entry, and the calendar address in the Keychain. This button cleans all of that up and opens your Applications folder, where you just drop Ptáček in the Trash. Your calendar and everything else stays untouched.",
    uninstallBtn: "Uninstall Ptáček…",
    uninstallConfirm:
      "Ptáček will delete its settings, turn off launch at login, remove the calendar address from the Keychain, and quit. Continue?",
    uninstallYes: "Yes, clean up and quit",
    uninstallCancel: "Cancel",
  },
};

export type Strings = (typeof STRINGS)["cs"];

export function getStrings(lang: Lang): Strings {
  return STRINGS[lang] ?? STRINGS.cs;
}
