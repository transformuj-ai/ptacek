use log::{error, info};
use tauri::WindowUrl;

pub fn open_setting_window(app: tauri::AppHandle) {
    // Okno už existuje (souběh onboarding × klik v tray) → jen fokus.
    use tauri::Manager;
    if let Some(existing) = app.get_window("setting") {
        let _ = existing.set_focus();
        return;
    }

    // Ptáček je vždy tmavý (brand), takže tu na rozdíl od forku není
    // potřeba číst AppConfig kvůli tématu.
    match tauri::WindowBuilder::new(&app, "setting", WindowUrl::App("/setting".into()))
        .title("Ptáček by Transformuj.ai — nastavení")
        .inner_size(760.0, 820.0)
        .min_inner_size(640.0, 480.0)
        .theme(Some(tauri::Theme::Dark))
        .build()
    {
        Ok(_window) => info!("Otevřeno okno nastavení"),
        // Nikdy nepanikaříme kvůli selhání vytvoření okna — jen zalogujeme.
        Err(err) => error!("Failed to create setting window: {}", err),
    }
}
