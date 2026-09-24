mod usage;
mod widget_preferences;

use tauri::{Manager, PhysicalPosition, Position};

#[tauri::command]
fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

fn widget_position(app: &tauri::AppHandle, saved: Option<widget_preferences::WidgetPosition>, size: (i32, i32)) -> Result<PhysicalPosition<i32>, String> {
    let monitors = app.available_monitors().map_err(|error| error.to_string())?;
    if let Some(position) = saved {
        let visible = monitors.iter().any(|monitor| {
            let origin = monitor.position();
            let monitor_size = monitor.size();
            position.x >= origin.x
                && position.y >= origin.y
                && position.x.saturating_add(size.0) <= origin.x.saturating_add(monitor_size.width as i32)
                && position.y.saturating_add(size.1) <= origin.y.saturating_add(monitor_size.height as i32)
        });
        if visible {
            return Ok(PhysicalPosition::new(position.x, position.y));
        }
    }

    let primary = app.primary_monitor().map_err(|error| error.to_string())?
        .or_else(|| monitors.first().cloned());
    if let Some(monitor) = primary {
        let origin = monitor.position();
        let monitor_size = monitor.size();
        return Ok(PhysicalPosition::new(
            origin.x + (monitor_size.width as i32 - size.0).max(0) / 2,
            origin.y + (monitor_size.height as i32 - size.1).max(0) / 2,
        ));
    }
    Ok(PhysicalPosition::new(0, 0))
}

fn configure_widget_windows(app: &tauri::AppHandle) -> Result<(), String> {
    let preferences = widget_preferences::load(app)?;
    let large_window = app.get_webview_window("large").ok_or_else(|| "Janela large não foi criada.".to_string())?;
    let compact_window = app.get_webview_window("compact").ok_or_else(|| "Janela compact não foi criada.".to_string())?;
    let large_size = large_window.outer_size().map_err(|error| error.to_string())?;
    let compact_size = compact_window.outer_size().map_err(|error| error.to_string())?;
    let large_dimensions = (large_size.width as i32, large_size.height as i32);
    let compact_dimensions = (compact_size.width as i32, compact_size.height as i32);
    let large_position = widget_position(app, preferences.large_position, large_dimensions)?;
    let compact_position = if preferences.compact_position.is_some() {
        widget_position(app, preferences.compact_position, compact_dimensions)?
    } else {
        position_near_widget(app, large_position, large_dimensions, compact_dimensions)?
    };
    large_window.set_position(Position::Physical(large_position)).map_err(|error| error.to_string())?;
    compact_window.set_position(Position::Physical(compact_position)).map_err(|error| error.to_string())?;
    for (label, position) in [("large", large_position), ("compact", compact_position)] {
        let saved = widget_preferences::WidgetPosition { x: position.x, y: position.y };
        widget_preferences::save_startup_position(app, label, saved)?;
    }
    let show_large = matches!(preferences.mode, widget_preferences::WidgetMode::Large);
    let show_compact = matches!(preferences.mode, widget_preferences::WidgetMode::Compact);
    if show_large { app.get_webview_window("large").unwrap().show().map_err(|error| error.to_string())?; }
    if show_compact { app.get_webview_window("compact").unwrap().show().map_err(|error| error.to_string())?; }
    Ok(())
}

fn position_near_widget(
    app: &tauri::AppHandle,
    large_position: PhysicalPosition<i32>,
    large_size: (i32, i32),
    compact_size: (i32, i32),
) -> Result<PhysicalPosition<i32>, String> {
    let monitors = app.available_monitors().map_err(|error| error.to_string())?;
    let center_x = large_position.x + large_size.0 / 2;
    let center_y = large_position.y + large_size.1 / 2;
    let monitor = monitors.iter().find(|monitor| {
        let origin = monitor.position();
        let size = monitor.size();
        center_x >= origin.x && center_x < origin.x + size.width as i32
            && center_y >= origin.y && center_y < origin.y + size.height as i32
    }).cloned().or_else(|| monitors.first().cloned());
    let Some(monitor) = monitor else { return Ok(PhysicalPosition::new(0, 0)); };
    let origin = monitor.position();
    let size = monitor.size();
    let right = origin.x + size.width as i32;
    let bottom = origin.y + size.height as i32;
    let candidates = [
        PhysicalPosition::new(large_position.x + large_size.0 + 16, large_position.y),
        PhysicalPosition::new(large_position.x - compact_size.0 - 16, large_position.y),
        PhysicalPosition::new(large_position.x + large_size.0 - compact_size.0, bottom - compact_size.1 - 16),
    ];
    for candidate in candidates {
        if candidate.x >= origin.x && candidate.y >= origin.y
            && candidate.x + compact_size.0 <= right && candidate.y + compact_size.1 <= bottom {
            return Ok(candidate);
        }
    }
    Ok(PhysicalPosition::new(
        origin.x + (size.width as i32 - compact_size.0).max(0) / 2,
        origin.y + (size.height as i32 - compact_size.1).max(0) / 2,
    ))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            widget_preferences::initialize_state(app).map_err(std::io::Error::other)?;
            configure_widget_windows(app.handle()).map_err(std::io::Error::other)?;
            usage::start_local_watcher(app.handle().clone());
            usage::start_lock_screen_publisher();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            exit_app,
            widget_preferences::get_widget_preferences,
            widget_preferences::set_widget_mode,
            widget_preferences::save_widget_position,
            usage::get_usage_dashboard,
            usage::get_codex_quota,
            usage::get_claude_quota,
            usage::clear_usage_history,
            usage::refresh_usage_sources,
            usage::get_usage_source_settings,
            usage::set_usage_source_enabled,
            usage::save_provider_credential,
            usage::provider_credential_status,
            usage::delete_provider_credential
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
