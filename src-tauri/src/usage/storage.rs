use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use tauri::{AppHandle, Manager};

use super::model::{SourceStatus, UsageBucket, UsageDashboard};

fn connection(app: &AppHandle) -> Result<Connection, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    Connection::open(directory.join("usage.sqlite")).map_err(|error| error.to_string())
}

pub fn initialize(app: &AppHandle) -> Result<(), String> {
    let database = connection(app)?;
    database
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS usage_buckets (
                 id TEXT PRIMARY KEY NOT NULL,
                 source_id TEXT NOT NULL,
                 usage_kind TEXT NOT NULL,
                 period_start INTEGER NOT NULL,
                 period_end INTEGER NOT NULL,
                 model TEXT,
                 session_key TEXT,
                 input_tokens INTEGER,
                 output_tokens INTEGER,
                 cached_input_tokens INTEGER,
                 total_tokens INTEGER,
                 amount REAL,
                 currency TEXT,
                 confidence TEXT NOT NULL,
                 observed_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS usage_buckets_period
                 ON usage_buckets(period_start);
             CREATE TABLE IF NOT EXISTS source_status (
                 source_id TEXT PRIMARY KEY NOT NULL,
                 state TEXT NOT NULL,
                 last_successful_refresh INTEGER,
                 message TEXT
             );
             CREATE TABLE IF NOT EXISTS source_checkpoints (
                 source_id TEXT PRIMARY KEY NOT NULL,
                 checkpoint TEXT
             );
             CREATE TABLE IF NOT EXISTS source_settings (
                 source_id TEXT PRIMARY KEY NOT NULL,
                 enabled INTEGER NOT NULL DEFAULT 1
             );
             DELETE FROM source_status WHERE source_id = 'chatgpt_work';
             DELETE FROM source_settings WHERE source_id = 'chatgpt_work';",
        )
        .map_err(|error| error.to_string())
}

pub fn upsert_bucket(app: &AppHandle, bucket: &UsageBucket) -> Result<(), String> {
    let database = connection(app)?;
    database
        .execute(
            "INSERT INTO usage_buckets (
                id, source_id, usage_kind, period_start, period_end, model,
                session_key, input_tokens, output_tokens, cached_input_tokens,
                total_tokens, amount, currency, confidence, observed_at
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15
             ) ON CONFLICT(id) DO UPDATE SET
                period_start=excluded.period_start,
                period_end=excluded.period_end,
                input_tokens=excluded.input_tokens,
                output_tokens=excluded.output_tokens,
                cached_input_tokens=excluded.cached_input_tokens,
                total_tokens=excluded.total_tokens,
                amount=excluded.amount,
                currency=excluded.currency,
                confidence=excluded.confidence,
                observed_at=excluded.observed_at",
            params![
                bucket.id,
                bucket.source_id,
                bucket.usage_kind,
                bucket.period_start,
                bucket.period_end,
                bucket.model,
                bucket.session_key,
                bucket.input_tokens,
                bucket.output_tokens,
                bucket.cached_input_tokens,
                bucket.total_tokens,
                bucket.amount,
                bucket.currency,
                bucket.confidence,
                bucket.observed_at,
            ],
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub fn save_source_status(app: &AppHandle, status: &SourceStatus) -> Result<(), String> {
    let database = connection(app)?;
    database
        .execute(
            "INSERT INTO source_status(source_id, state, last_successful_refresh, message)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(source_id) DO UPDATE SET
                state=excluded.state,
                last_successful_refresh=COALESCE(excluded.last_successful_refresh, source_status.last_successful_refresh),
                message=excluded.message",
            params![
                status.source_id,
                status.state,
                status.last_successful_refresh,
                status.message,
            ],
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub fn load_dashboard(app: &AppHandle) -> Result<UsageDashboard, String> {
    let database = connection(app)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_secs() as i64;
    let start = now - 7 * 24 * 60 * 60;
    let mut bucket_statement = database
        .prepare(
            "SELECT id, source_id, usage_kind, period_start, period_end, model,
                    session_key, input_tokens, output_tokens, cached_input_tokens,
                    total_tokens, amount, currency, confidence, observed_at
             FROM usage_buckets WHERE period_start >= ?1 ORDER BY period_start",
        )
        .map_err(|error| error.to_string())?;
    let buckets = bucket_statement
        .query_map([start], |row| {
            Ok(UsageBucket {
                id: row.get(0)?,
                source_id: row.get(1)?,
                usage_kind: row.get(2)?,
                period_start: row.get(3)?,
                period_end: row.get(4)?,
                model: row.get(5)?,
                session_key: row.get(6)?,
                input_tokens: row.get(7)?,
                output_tokens: row.get(8)?,
                cached_input_tokens: row.get(9)?,
                total_tokens: row.get(10)?,
                amount: row.get(11)?,
                currency: row.get(12)?,
                confidence: row.get(13)?,
                observed_at: row.get(14)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;

    let mut status_statement = database
        .prepare(
            "SELECT source_id, state, last_successful_refresh, message FROM source_status ORDER BY source_id",
        )
        .map_err(|error| error.to_string())?;
    let mut sources = status_statement
        .query_map([], |row| {
            Ok(SourceStatus {
                source_id: row.get(0)?,
                state: row.get(1)?,
                last_successful_refresh: row.get(2)?,
                message: row.get(3)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;

    for source_id in ["codex_local", "claude_code_local", "openai_api", "anthropic_api"] {
        if sources.iter().all(|source| source.source_id != source_id) {
            sources.push(SourceStatus {
                source_id: source_id.to_string(),
                state: "unavailable".to_string(),
                last_successful_refresh: None,
                message: Some("Ainda não foi conectado.".to_string()),
            });
        }
    }

    Ok(UsageDashboard {
        generated_at: now,
        buckets,
        sources,
    })
}

pub fn get_checkpoint(app: &AppHandle, source_id: &str) -> Result<Option<String>, String> {
    let database = connection(app)?;
    database
        .query_row(
            "SELECT checkpoint FROM source_checkpoints WHERE source_id = ?1",
            [source_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map(Option::flatten)
        .map_err(|error| error.to_string())
}

pub fn save_checkpoint(
    app: &AppHandle,
    source_id: &str,
    checkpoint: Option<&str>,
) -> Result<(), String> {
    let database = connection(app)?;
    database
        .execute(
            "INSERT INTO source_checkpoints(source_id, checkpoint) VALUES (?1, ?2)
             ON CONFLICT(source_id) DO UPDATE SET checkpoint=excluded.checkpoint",
            params![source_id, checkpoint],
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub fn source_enabled(app: &AppHandle, source_id: &str) -> Result<bool, String> {
    let database = connection(app)?;
    database
        .query_row(
            "SELECT enabled FROM source_settings WHERE source_id = ?1",
            [source_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map(|value| value.unwrap_or(1) != 0)
        .map_err(|error| error.to_string())
}

pub fn source_settings(app: &AppHandle) -> Result<std::collections::HashMap<String, bool>, String> {
    let database = connection(app)?;
    let mut settings = std::collections::HashMap::new();
    let mut statement = database
        .prepare("SELECT source_id, enabled FROM source_settings")
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? != 0)))
        .map_err(|error| error.to_string())?;
    for row in rows {
        let (source_id, enabled) = row.map_err(|error| error.to_string())?;
        settings.insert(source_id, enabled);
    }
    for source_id in ["codex_local", "claude_code_local", "openai_api", "anthropic_api"] {
        settings.entry(source_id.to_string()).or_insert(true);
    }
    Ok(settings)
}

pub fn set_source_enabled(app: &AppHandle, source_id: &str, enabled: bool) -> Result<(), String> {
    let database = connection(app)?;
    database
        .execute(
            "INSERT INTO source_settings(source_id, enabled) VALUES (?1, ?2)
             ON CONFLICT(source_id) DO UPDATE SET enabled=excluded.enabled",
            params![source_id, if enabled { 1 } else { 0 }],
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub fn clear_history(app: &AppHandle) -> Result<(), String> {
    let database = connection(app)?;
    database
        .execute_batch(
            "DELETE FROM usage_buckets;
             DELETE FROM source_checkpoints;",
        )
        .map_err(|error| error.to_string())
}
