mod claude_code;
mod codex;
mod openai_api;
mod anthropic_api;

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::DateTime;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use super::model::{CollectorResult, RefreshRequest, SourceStatus, UsageBucket};
use super::storage;

const SOURCES: [&str; 2] = ["codex_local", "claude_code_local"];

#[derive(Clone, Debug)]
struct FileProgress {
    offset: u64,
    session_id: Option<String>,
    session_start: Option<i64>,
}

#[derive(Default, Deserialize, Serialize)]
struct Checkpoint {
    files: HashMap<String, FileProgressRecord>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileProgressRecord {
    offset: u64,
    session_id: Option<String>,
    session_start: Option<i64>,
}

trait UsageCollector {
    fn source_id(&self) -> &'static str;
    fn discover(&self) -> Result<Vec<PathBuf>, String>;
    fn collect(
        &self,
        app: &AppHandle,
        files: &[PathBuf],
        request: &RefreshRequest,
    ) -> CollectorResult;
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

fn source_status(source_id: &str, state: &str, message: Option<String>) -> SourceStatus {
    SourceStatus {
        source_id: source_id.to_string(),
        state: state.to_string(),
        last_successful_refresh: if state == "connected" || state == "partial" {
            Some(now())
        } else {
            None
        },
        message,
    }
}

fn parse_timestamp(value: Option<&str>) -> Option<i64> {
    value.and_then(|value| {
        DateTime::parse_from_rfc3339(value)
            .ok()
            .map(|timestamp| timestamp.timestamp())
    })
}

fn file_key(path: &Path) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn read_checkpoint(app: &AppHandle, source_id: &str) -> Checkpoint {
    storage::get_checkpoint(app, source_id)
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

fn result_with_checkpoint(
    app: &AppHandle,
    source_id: &str,
    buckets: Vec<UsageBucket>,
    status: SourceStatus,
    checkpoint: Checkpoint,
) -> CollectorResult {
    if let Ok(value) = serde_json::to_string(&checkpoint) {
        let _ = storage::save_checkpoint(app, source_id, Some(&value));
    }
    CollectorResult {
        buckets,
        status,
    }
}

fn walk_jsonl(root: &Path, max_depth: usize) -> Vec<PathBuf> {
    fn walk(path: &Path, depth: usize, max_depth: usize, result: &mut Vec<PathBuf>) {
        if depth > max_depth {
            return;
        }
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, depth + 1, max_depth, result);
            } else if path.extension().is_some_and(|extension| extension == "jsonl") {
                result.push(path);
            }
        }
    }

    let mut result = Vec::new();
    walk(root, 0, max_depth, &mut result);
    result
}

fn resolve_root(env_name: &str, default_leaf: &str) -> Option<PathBuf> {
    if let Some(configured) = std::env::var_os(env_name) {
        return Some(PathBuf::from(configured));
    }
    std::env::var_os("USERPROFILE").map(|profile| PathBuf::from(profile).join(default_leaf))
}

pub fn refresh_local(app: &AppHandle, request: &RefreshRequest) -> Vec<SourceStatus> {
    let mut statuses = Vec::new();

    let collectors: Vec<Box<dyn UsageCollector>> = vec![
        Box::new(codex::CodexCollector),
        Box::new(claude_code::ClaudeCodeCollector),
    ];

    for collector in collectors {
        if !storage::source_enabled(app, collector.source_id()).unwrap_or(true) {
            let status = source_status(
                collector.source_id(),
                "unavailable",
                Some("Coleta desativada nas configurações.".to_string()),
            );
            let _ = storage::save_source_status(app, &status);
            statuses.push(status);
            continue;
        }
        let status = match collector.discover() {
            Ok(files) if files.is_empty() => source_status(
                collector.source_id(),
                "unavailable",
                Some("Nenhuma sessão local encontrada.".to_string()),
            ),
            Ok(files) => {
                let result = collector.collect(app, &files, request);
                for bucket in &result.buckets {
                    let _ = storage::upsert_bucket(app, bucket);
                }
                let _ = storage::save_source_status(app, &result.status);
                result.status
            }
            Err(message) => source_status(collector.source_id(), "unavailable", Some(message)),
        };
        let _ = storage::save_source_status(app, &status);
        statuses.push(status);
    }

    for source_id in SOURCES {
        if statuses.iter().all(|status| status.source_id != source_id) {
            let status = source_status(source_id, "unavailable", Some("Fonte indisponível.".into()));
            let _ = storage::save_source_status(app, &status);
            statuses.push(status);
        }
    }
    statuses
}

pub fn refresh(app: &AppHandle, start_time_unix: i64, end_time_unix: i64) -> Vec<SourceStatus> {
    let request = RefreshRequest { start_time_unix, end_time_unix };
    let mut statuses = refresh_local(app, &request);
    for result in [openai_api::collect(app, &request), anthropic_api::collect(app, &request)] {
        for bucket in &result.buckets {
            let _ = storage::upsert_bucket(app, bucket);
        }
        let _ = storage::save_source_status(app, &result.status);
        statuses.push(result.status);
    }
    statuses
}

pub fn default_window() -> (i64, i64) {
    (now() - 30 * 24 * 60 * 60, now())
}

pub fn watch_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(root) = resolve_root("CODEX_HOME", ".codex") {
        let sessions = root.join("sessions");
        if sessions.is_dir() {
            roots.push(sessions);
        }
    }
    if let Some(root) = resolve_root("CLAUDE_CONFIG_DIR", ".claude") {
        let projects = root.join("projects");
        if projects.is_dir() {
            roots.push(projects);
        }
    }
    roots
}

fn file_progress(checkpoint: &Checkpoint, path: &Path) -> FileProgress {
    checkpoint
        .files
        .get(&file_key(path))
        .map(|record| FileProgress {
            offset: record.offset,
            session_id: record.session_id.clone(),
            session_start: record.session_start,
        })
        .unwrap_or(FileProgress {
            offset: 0,
            session_id: None,
            session_start: None,
        })
}

fn update_file_progress(checkpoint: &mut Checkpoint, path: &Path, progress: FileProgress) {
    checkpoint.files.insert(
        file_key(path),
        FileProgressRecord {
            offset: progress.offset,
            session_id: progress.session_id,
            session_start: progress.session_start,
        },
    );
}

fn bucket_id(source_id: &str, key: &str) -> String {
    format!("{source_id}:{key}")
}
