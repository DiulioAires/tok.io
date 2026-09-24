use std::{fs, path::PathBuf, sync::Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WidgetMode {
    #[default]
    Large,
    Compact,
    Both,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct WidgetPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WidgetPreferences {
    pub mode: WidgetMode,
    pub large_position: Option<WidgetPosition>,
    pub compact_position: Option<WidgetPosition>,
}

impl Default for WidgetPreferences {
    fn default() -> Self {
        Self {
            mode: WidgetMode::Large,
            large_position: None,
            compact_position: None,
        }
    }
}

pub struct WidgetPreferencesState(pub Mutex<WidgetPreferences>);

fn file_path(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app.path().app_data_dir().map_err(|error| error.to_string())?;
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    Ok(directory.join("widget-preferences.json"))
}

pub fn load(app: &AppHandle) -> Result<WidgetPreferences, String> {
    let path = file_path(app)?;
    match fs::read_to_string(path) {
        Ok(contents) => {
            let mut preferences: WidgetPreferences = serde_json::from_str(&contents).unwrap_or_default();
            if matches!(preferences.mode, WidgetMode::Both) {
                preferences.mode = WidgetMode::Large;
                persist(app, &preferences)?;
            }
            Ok(preferences)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(WidgetPreferences::default()),
        Err(error) => Err(error.to_string()),
    }
}

fn persist(app: &AppHandle, preferences: &WidgetPreferences) -> Result<(), String> {
    let path = file_path(app)?;
    let temporary = path.with_extension("json.tmp");
    let contents = serde_json::to_vec_pretty(preferences).map_err(|error| error.to_string())?;
    fs::write(&temporary, contents).map_err(|error| error.to_string())?;
    if path.exists() {
        fs::remove_file(&path).map_err(|error| error.to_string())?;
    }
    fs::rename(temporary, path).map_err(|error| error.to_string())
}

fn update<F>(app: &AppHandle, state: &WidgetPreferencesState, change: F) -> Result<WidgetPreferences, String>
where
    F: FnOnce(&mut WidgetPreferences),
{
    let mut preferences = state.0.lock().map_err(|_| "Não foi possível acessar as preferências do widget.".to_string())?;
    change(&mut preferences);
    persist(app, &preferences)?;
    Ok(preferences.clone())
}

pub fn save_startup_position(app: &AppHandle, label: &str, position: WidgetPosition) -> Result<(), String> {
    let state = app.state::<WidgetPreferencesState>();
    update(app, &state, |preferences| match label {
        "large" => preferences.large_position = Some(position),
        "compact" => preferences.compact_position = Some(position),
        _ => {}
    }).map(|_| ())
}

#[tauri::command]
pub fn get_widget_preferences(state: State<'_, WidgetPreferencesState>) -> Result<WidgetPreferences, String> {
    state.0.lock()
        .map(|preferences| preferences.clone())
        .map_err(|_| "Não foi possível carregar as preferências do widget.".to_string())
}

#[tauri::command]
pub fn set_widget_mode(app: AppHandle, state: State<'_, WidgetPreferencesState>, mode: WidgetMode) -> Result<WidgetPreferences, String> {
    let mode = if matches!(mode, WidgetMode::Both) { WidgetMode::Large } else { mode };
    let preferences = update(&app, &state, |preferences| preferences.mode = mode)?;
    let large_visible = matches!(mode, WidgetMode::Large);
    let compact_visible = matches!(mode, WidgetMode::Compact);
    if let Some(window) = app.get_webview_window("large") {
        if large_visible { window.show().map_err(|error| error.to_string())?; }
        else { window.hide().map_err(|error| error.to_string())?; }
    }
    if let Some(window) = app.get_webview_window("compact") {
        if compact_visible { window.show().map_err(|error| error.to_string())?; }
        else { window.hide().map_err(|error| error.to_string())?; }
    }
    app.emit("widget-mode-updated", mode).map_err(|error| error.to_string())?;
    Ok(preferences)
}

#[tauri::command]
pub fn save_widget_position(
    app: AppHandle,
    state: State<'_, WidgetPreferencesState>,
    window_label: String,
    position: WidgetPosition,
) -> Result<WidgetPreferences, String> {
    if window_label != "large" && window_label != "compact" {
        return Err("Janela desconhecida.".to_string());
    }
    update(&app, &state, |preferences| match window_label.as_str() {
        "large" => preferences.large_position = Some(position),
        "compact" => preferences.compact_position = Some(position),
        _ => {}
    })
}

pub fn initialize_state(app: &mut tauri::App) -> Result<(), String> {
    let preferences = load(app.handle())?;
    app.manage(WidgetPreferencesState(Mutex::new(preferences)));
    Ok(())
}
