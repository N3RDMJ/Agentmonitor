use tauri::{State, Window};

use crate::state::AppState;
use crate::shared::cli_detect_core::{self, DetectedClis};
use crate::shared::settings_core::{
    get_app_settings_core, get_codex_config_path_core, update_app_settings_core,
};
use crate::types::AppSettings;
use crate::window;

#[tauri::command]
pub(crate) async fn get_app_settings(
    state: State<'_, AppState>,
    window: Window,
) -> Result<AppSettings, String> {
    let settings = get_app_settings_core(&state.app_settings).await;
    let _ = window::apply_window_appearance(&window, settings.theme.as_str());
    Ok(settings)
}

#[tauri::command]
pub(crate) async fn update_app_settings(
    settings: AppSettings,
    state: State<'_, AppState>,
    window: Window,
) -> Result<AppSettings, String> {
    let updated =
        update_app_settings_core(settings, &state.app_settings, &state.settings_path).await?;
    let _ = window::apply_window_appearance(&window, updated.theme.as_str());
    Ok(updated)
}

#[tauri::command]
pub(crate) async fn get_codex_config_path() -> Result<String, String> {
    get_codex_config_path_core()
}

#[tauri::command]
pub(crate) async fn detect_installed_clis() -> Result<DetectedClis, String> {
    Ok(cli_detect_core::detect_installed_clis().await)
}
