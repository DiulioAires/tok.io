use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use reqwest::blocking::Client;
use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeQuotaWindow {
    pub used_percent: f64,
    pub resets_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeQuota {
    pub five_hour: Option<ClaudeQuotaWindow>,
    pub weekly: Option<ClaudeQuotaWindow>,
    pub refreshed_at: i64,
}

struct RequestState {
    token_fingerprint: u64,
    next_allowed_at: Option<Instant>,
    backoff: Duration,
    cached: Option<(Instant, ClaudeQuota)>,
}

static REQUEST_STATE: OnceLock<Mutex<Option<RequestState>>> = OnceLock::new();
static CLAUDE_USER_AGENT: OnceLock<String> = OnceLock::new();

fn credentials_path() -> Option<PathBuf> {
    let custom_dir = std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from);
    let root = custom_dir
        .clone()
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))?;
    let config_dir = if custom_dir.is_some() {
        root
    } else {
        root.join(".claude")
    };
    Some(config_dir.join(".credentials.json"))
}

fn access_token() -> Result<String, String> {
    if let Some(token) = std::env::var("CLAUDE_CODE_OAUTH_TOKEN")
        .ok()
        .filter(|token| !token.is_empty())
    {
        return Ok(token);
    }
    let path = credentials_path().ok_or_else(|| {
        "Não foi possível localizar a sessão do Claude Code neste computador.".to_string()
    })?;
    let contents = std::fs::read_to_string(path).map_err(|_| {
        "Faça login no Claude Code para consultar os limites da assinatura.".to_string()
    })?;
    let credentials: Value = serde_json::from_str(&contents)
        .map_err(|_| "Não foi possível ler as credenciais locais do Claude Code.".to_string())?;
    credentials
        .pointer("/claudeAiOauth/accessToken")
        .or_else(|| credentials.get("accessToken"))
        .and_then(Value::as_str)
        .filter(|token| !token.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "Não foi encontrada uma sessão Claude.ai. Entre no Claude Code com sua conta Pro ou Max.".to_string())
}

fn token_fingerprint(token: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    token.hash(&mut hasher);
    hasher.finish()
}

fn claude_user_agent() -> &'static str {
    CLAUDE_USER_AGENT
        .get_or_init(|| {
            let mut command = Command::new("claude");
            command.arg("--version");
            super::hide_child_window(&mut command);
            let version = command
                .output()
                .ok()
                .filter(|output| output.status.success())
                .and_then(|output| String::from_utf8(output.stdout).ok())
                .and_then(|output| output.split_whitespace().next().map(str::to_owned))
                .unwrap_or_else(|| "unknown".to_string());
            format!("claude-code/{version}")
        })
        .as_str()
}

fn reset_timestamp(value: &Value) -> Option<i64> {
    if let Some(timestamp) = value.as_i64() {
        return Some(timestamp);
    }
    let value = value.as_str()?;
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|datetime| datetime.timestamp())
}

fn window(value: &Value) -> Option<ClaudeQuotaWindow> {
    let utilization = value.get("utilization")?.as_f64()?;
    let used_percent = if utilization <= 1.0 {
        utilization * 100.0
    } else {
        utilization
    };
    Some(ClaudeQuotaWindow {
        used_percent: used_percent.clamp(0.0, 100.0),
        resets_at: reset_timestamp(value.get("resets_at")?)?,
    })
}

fn parse_quota(data: Value) -> Result<ClaudeQuota, String> {
    let five_hour = data.get("five_hour").and_then(window);
    let weekly = data.get("seven_day").and_then(window);
    if five_hour.is_none() && weekly.is_none() {
        return Err("A Anthropic não retornou janelas de 5 horas ou semanal.".to_string());
    }
    let refreshed_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default();
    Ok(ClaudeQuota {
        five_hour,
        weekly,
        refreshed_at,
    })
}

pub fn read() -> Result<ClaudeQuota, String> {
    let token = access_token()?;
    let fingerprint = token_fingerprint(&token);
    let state = REQUEST_STATE.get_or_init(|| Mutex::new(None));
    {
        let mut state = state
            .lock()
            .map_err(|_| "Não foi possível consultar os limites do Claude.".to_string())?;
        if state
            .as_ref()
            .is_some_and(|current| current.token_fingerprint != fingerprint)
        {
            *state = None;
        }
        if let Some(current) = state.as_ref() {
            if let Some((saved_at, quota)) = &current.cached {
                if saved_at.elapsed() < Duration::from_secs(180) {
                    return Ok(quota.clone());
                }
            }
            if current
                .next_allowed_at
                .is_some_and(|at| Instant::now() < at)
            {
                return Err("A consulta do Claude está em pausa após um limite temporário. Se continuar, execute `claude /login` para renovar a sessão e atualize o tok.io.".to_string());
            }
        }
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|_| "Não foi possível preparar a consulta ao Claude.".to_string())?;
    let response = client
        .get("https://api.anthropic.com/api/oauth/usage")
        .bearer_auth(&token)
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("User-Agent", claude_user_agent())
        .send()
        .map_err(|_| {
            "Não foi possível consultar os limites do Claude. Confira sua conexão.".to_string()
        })?;

    if response.status().as_u16() == 429 {
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .map(Duration::from_secs);
        let mut state = state
            .lock()
            .map_err(|_| "Não foi possível consultar os limites do Claude.".to_string())?;
        let current = state.get_or_insert(RequestState {
            token_fingerprint: fingerprint,
            next_allowed_at: None,
            backoff: Duration::from_secs(300),
            cached: None,
        });
        let delay = retry_after
            .unwrap_or(current.backoff)
            .max(Duration::from_secs(60));
        current.next_allowed_at = Some(Instant::now() + delay);
        current.backoff = (current.backoff * 2).min(Duration::from_secs(3600));
        return Err("A Anthropic limitou as consultas deste token. O TokenEater recomenda executar `claude /login` para renovar a sessão; depois atualize o tok.io.".to_string());
    }
    if !response.status().is_success() {
        return Err(match response.status().as_u16() {
            401 | 403 => {
                "A sessão do Claude Code expirou. Execute `claude /login` e atualize o tok.io."
                    .to_string()
            }
            _ => "A Anthropic não retornou os limites da assinatura. Tente novamente mais tarde."
                .to_string(),
        });
    }

    let data: Value = response
        .json()
        .map_err(|_| "A resposta de limites do Claude estava em formato inesperado.".to_string())?;
    let quota = parse_quota(data)?;
    let mut state = state
        .lock()
        .map_err(|_| "Não foi possível consultar os limites do Claude.".to_string())?;
    *state = Some(RequestState {
        token_fingerprint: fingerprint,
        next_allowed_at: Some(Instant::now() + Duration::from_secs(180)),
        backoff: Duration::from_secs(300),
        cached: Some((Instant::now(), quota.clone())),
    });
    Ok(quota)
}
