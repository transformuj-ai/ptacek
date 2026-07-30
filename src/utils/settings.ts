import { Store } from "tauri-plugin-store-api";
import { invoke } from "@tauri-apps/api/tauri";

// Odpovídá AppConfig v src-tauri/src/app/conf.rs — udržuj obě strany
// synchronizované. Sanitizace ICS URL / keychain přibude v PR9,
// autostart toggle v PR8.
export interface AppSettings {
    minutesBefore: number;
    mascot: string;
    calendarIds: string[];
    soundEnabled: boolean;
    launchAtLogin: boolean;
    icsUrlSet: boolean;
    muteUntil: number | null;
    /** násobič rychlosti přeletu (0.65 rychle · 1 normálně · 1.5 pomalu) */
    speed: number;
    /** jazyk UI: cs | en */
    language: string;
    /** onboarding při prvním spuštění už proběhl */
    firstRunDone: boolean;
    /** co maskot říká: title | fun | hybrid */
    textMode: string;
}

export const DEFAULT_SETTINGS: AppSettings = {
    minutesBefore: 2,
    mascot: "random",
    calendarIds: [],
    soundEnabled: false,
    launchAtLogin: false,
    icsUrlSet: false,
    muteUntil: null,
    speed: 1,
    language: "cs",
    firstRunDone: false,
    textMode: "hybrid",
};

async function getSettingsStorePath(): Promise<string> {
    const path = await invoke<string | null>("combine_config_path", {
        config_name: "settings.json",
    });

    if (!path) {
        throw new Error("Could not resolve settings.json path");
    }

    return path;
}

export async function getSettings(): Promise<AppSettings> {
    const path = await getSettingsStorePath();
    const store = new Store(path);
    const app = await store.get<Partial<AppSettings>>("app");
    return { ...DEFAULT_SETTINGS, ...(app ?? {}) };
}

export async function setSetting<K extends keyof AppSettings>(
    key: K,
    value: AppSettings[K]
): Promise<void> {
    const current = await getSettings();
    const next: AppSettings = { ...current, [key]: value };
    const path = await getSettingsStorePath();
    const store = new Store(path);
    await store.set("app", next);
    await store.save();
    // scheduler přeplánuje hned, ne až za 5 minut
    invoke("calendars_changed").catch(() => undefined);
}
