use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    pub used_percent: f64,
    pub resets_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexQuota {
    pub five_hour: Option<QuotaWindow>,
    pub weekly: Option<QuotaWindow>,
    pub plan_type: Option<String>,
    pub refreshed_at: i64,
}

fn codex_program() -> PathBuf {
    if let Some(path) = std::env::var_os("CODEX_CLI_PATH") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return path;
        }
    }

    #[cfg(windows)]
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let bin_dir = PathBuf::from(local_app_data).join("OpenAI").join("Codex").join("bin");
        if let Ok(entries) = std::fs::read_dir(bin_dir) {
            let mut candidates = entries
                .flatten()
                .map(|entry| entry.path().join("codex.exe"))
                .filter(|path| path.is_file())
                .collect::<Vec<_>>();
            candidates.sort_by_key(|path| {
                std::fs::metadata(path)
                    .and_then(|metadata| metadata.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH)
            });
            if let Some(path) = candidates.pop() {
                return path;
            }
        }
    }

    PathBuf::from("codex")
}

fn request_rate_limits() -> Result<Value, String> {
    let mut command = Command::new(codex_program());
    command
        .arg("app-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    super::hide_child_window(&mut command);
    let mut child = command
        .spawn()
        .map_err(|_| "Codex CLI não encontrado. Instale ou atualize o Codex para consultar o limite da conta.".to_string())?;

    let input = child.stdin.take().ok_or_else(|| "Não foi possível iniciar a consulta de limites.".to_string())?;
    let output = child.stdout.take().ok_or_else(|| "Não foi possível ler a resposta do Codex.".to_string())?;
    let (sender, receiver) = mpsc::channel::<String>();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(output).lines() {
            match line {
                Ok(line) => {
                    if sender.send(line).is_err() {
                        break;
                    }
                }
                _ => break,
            }
        }
    });

    let mut input = input;
    let messages = [
        serde_json::json!({
            "method": "initialize",
            "id": 1,
            "params": { "clientInfo": { "name": "ai_usage_widget", "title": "tok.io", "version": env!("CARGO_PKG_VERSION") } }
        }),
        serde_json::json!({ "method": "initialized", "params": {} }),
        serde_json::json!({ "method": "account/rateLimits/read", "id": 2 }),
    ];
    for message in messages {
        serde_json::to_writer(&mut input, &message).map_err(|_| "Não foi possível enviar a consulta ao Codex.".to_string())?;
        input.write_all(b"\n").map_err(|_| "Não foi possível enviar a consulta ao Codex.".to_string())?;
    }
    input.flush().map_err(|_| "Não foi possível enviar a consulta ao Codex.".to_string())?;

    let deadline = Instant::now() + Duration::from_secs(15);
    let response = loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break Err("A consulta dos limites da conta Codex demorou demais.".to_string());
        }
        match receiver.recv_timeout(remaining) {
            Ok(line) => {
                let Ok(message) = serde_json::from_str::<Value>(&line) else { continue };
                if message.get("id").and_then(Value::as_i64) != Some(2) {
                    continue;
                }
                if let Some(error) = message.get("error") {
                    let detail = error.get("message").and_then(Value::as_str).unwrap_or_default();
                    break Err(if detail.is_empty() {
                        "Não foi possível ler os limites da conta Codex. Confirme que o Codex está conectado à sua conta ChatGPT.".to_string()
                    } else {
                        format!("Não foi possível ler os limites da conta Codex: {detail}")
                    });
                }
                break message.get("result").cloned().ok_or_else(|| "O Codex não retornou os limites da conta.".to_string());
            }
            Err(mpsc::RecvTimeoutError::Timeout) => break Err("A consulta dos limites da conta Codex demorou demais.".to_string()),
            Err(mpsc::RecvTimeoutError::Disconnected) => break Err("O Codex encerrou a consulta antes de retornar os limites.".to_string()),
        }
    };
    let _ = child.kill();
    let _ = child.wait();
    let _ = reader.join();
    response
}

fn window(value: &Value, expected_minutes: i64) -> Option<QuotaWindow> {
    let duration = value.get("windowDurationMins")?.as_i64()?;
    if duration != expected_minutes {
        return None;
    }
    let used_percent = value.get("usedPercent")?.as_f64()?.clamp(0.0, 100.0);
    let resets_at = value.get("resetsAt")?.as_i64()?;
    Some(QuotaWindow { used_percent, resets_at })
}

pub fn read() -> Result<CodexQuota, String> {
    let result = request_rate_limits()?;
    let limits = result
        .get("rateLimitsByLimitId")
        .and_then(|limits| limits.get("codex"))
        .or_else(|| result.get("rateLimits"))
        .ok_or_else(|| "A conta Codex não retornou informações dos limites.".to_string())?;
    let primary = limits.get("primary").unwrap_or(&Value::Null);
    let secondary = limits.get("secondary").unwrap_or(&Value::Null);
    let windows = [primary, secondary];
    let five_hour = windows.iter().find_map(|limit| window(limit, 300));
    let weekly = windows.iter().find_map(|limit| window(limit, 10_080));
    if five_hour.is_none() && weekly.is_none() {
        return Err("A conta não retornou janelas de 5 horas ou semanal.".to_string());
    }
    let refreshed_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default();
    Ok(CodexQuota {
        five_hour,
        weekly,
        plan_type: limits.get("planType").and_then(Value::as_str).map(str::to_string),
        refreshed_at,
    })
}
