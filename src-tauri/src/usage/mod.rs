mod model;
mod storage;
mod collectors;
mod credentials;
mod codex_quota;
mod claude_quota;
mod lock_screen;

pub use model::{SourceStatus, UsageDashboard};

pub fn start_lock_screen_publisher() {
    lock_screen::start();
}

pub fn start_local_watcher(app: tauri::AppHandle) {
    use notify::{RecursiveMode, Watcher};
    use std::sync::mpsc;
    use std::time::Duration;

    let roots = collectors::watch_roots();
    if roots.is_empty() {
        return;
    }

    std::thread::spawn(move || {
        let (sender, receiver) = mpsc::channel();
        let Ok(mut watcher) = notify::recommended_watcher(move |event| {
            let _ = sender.send(event);
        }) else {
            return;
        };
        for root in roots {
            let _ = watcher.watch(&root, RecursiveMode::Recursive);
        }

        loop {
            if receiver.recv().is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
            while receiver.try_recv().is_ok() {}
            if storage::initialize(&app).is_ok() {
                let (start, end) = collectors::default_window();
                collectors::refresh_local(&app, &model::RefreshRequest { start_time_unix: start, end_time_unix: end });
            }
        }
    });
}

#[tauri::command]
pub fn get_usage_dashboard(
    app: tauri::AppHandle,
) -> Result<UsageDashboard, String> {
    storage::initialize(&app).map_err(|_| "Não foi possível abrir o histórico local.".to_string())?;
    storage::load_dashboard(&app)
        .map_err(|_| "Não foi possível carregar os dados de uso.".to_string())
}

#[tauri::command]
pub async fn get_codex_quota() -> Result<codex_quota::CodexQuota, String> {
    tauri::async_runtime::spawn_blocking(codex_quota::read)
        .await
        .map_err(|_| "Não foi possível consultar os limites da conta.".to_string())?
}

#[tauri::command]
pub async fn get_claude_quota() -> Result<claude_quota::ClaudeQuota, String> {
    tauri::async_runtime::spawn_blocking(claude_quota::read)
        .await
        .map_err(|_| "Não foi possível consultar os limites do Claude.".to_string())?
}

#[tauri::command]
pub fn clear_usage_history(app: tauri::AppHandle) -> Result<(), String> {
    storage::initialize(&app).map_err(|_| "Não foi possível abrir o histórico local.".to_string())?;
    storage::clear_history(&app)
        .map_err(|_| "Não foi possível apagar o histórico local.".to_string())
}

#[tauri::command]
pub fn get_usage_source_settings(
    app: tauri::AppHandle,
) -> Result<std::collections::HashMap<String, bool>, String> {
    storage::initialize(&app).map_err(|_| "Não foi possível abrir as configurações locais.".to_string())?;
    storage::source_settings(&app)
        .map_err(|_| "Não foi possível carregar as configurações das fontes.".to_string())
}

#[tauri::command]
pub fn set_usage_source_enabled(
    app: tauri::AppHandle,
    source_id: String,
    enabled: bool,
) -> Result<(), String> {
    if !["codex_local", "claude_code_local", "openai_api", "anthropic_api"]
        .contains(&source_id.as_str())
    {
        return Err("Fonte desconhecida.".to_string());
    }
    storage::initialize(&app).map_err(|_| "Não foi possível abrir as configurações locais.".to_string())?;
    storage::set_source_enabled(&app, &source_id, enabled)
        .map_err(|_| "Não foi possível atualizar a fonte.".to_string())
}

#[tauri::command]
pub fn save_provider_credential(provider: String, secret: String) -> Result<(), String> {
    credentials::save(&provider, &secret)
}

#[tauri::command]
pub fn provider_credential_status() -> std::collections::HashMap<String, bool> {
    ["openai", "anthropic"]
        .into_iter()
        .map(|provider| (provider.to_string(), credentials::configured(provider)))
        .collect()
}

#[tauri::command]
pub fn delete_provider_credential(provider: String) -> Result<(), String> {
    credentials::delete(&provider)
}

#[tauri::command]
pub async fn refresh_usage_sources(
    app: tauri::AppHandle,
    start_time_unix: Option<i64>,
    end_time_unix: Option<i64>,
) -> Result<Vec<SourceStatus>, String> {
    storage::initialize(&app).map_err(|_| "Não foi possível abrir o histórico local.".to_string())?;
    let (default_start, default_end) = collectors::default_window();
    tauri::async_runtime::spawn_blocking(move || {
        collectors::refresh(&app, start_time_unix.unwrap_or(default_start), end_time_unix.unwrap_or(default_end))
    }).await.map_err(|_| "Não foi possível atualizar as fontes.".to_string())
}
