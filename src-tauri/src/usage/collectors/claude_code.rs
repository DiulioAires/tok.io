use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;

use serde_json::Value;
use tauri::AppHandle;

use super::super::model::{CollectorResult, RefreshRequest, UsageBucket};
use super::{
    bucket_id, file_progress, read_checkpoint, result_with_checkpoint, source_status,
    update_file_progress, UsageCollector,
};

pub struct ClaudeCodeCollector;

impl UsageCollector for ClaudeCodeCollector {
    fn source_id(&self) -> &'static str {
        "claude_code_local"
    }

    fn discover(&self) -> Result<Vec<PathBuf>, String> {
        let Some(root) = super::resolve_root("CLAUDE_CONFIG_DIR", ".claude") else {
            return Err("Não foi possível localizar a pasta do usuário do Windows.".into());
        };
        let projects = root.join("projects");
        if !projects.is_dir() {
            return Ok(Vec::new());
        }
        Ok(super::walk_jsonl(&projects, 8))
    }

    fn collect(
        &self,
        app: &AppHandle,
        files: &[PathBuf],
        request: &RefreshRequest,
    ) -> CollectorResult {
        let source_id = self.source_id();
        let mut checkpoint = read_checkpoint(app, source_id);
        let mut buckets = Vec::new();
        let mut parse_errors = 0usize;

        for path in files {
            let Ok(file) = std::fs::File::open(path) else {
                parse_errors += 1;
                continue;
            };
            let Ok(metadata) = file.metadata() else {
                parse_errors += 1;
                continue;
            };
            let old = file_progress(&checkpoint, path);
            let mut reader = BufReader::new(file);
            let mut offset = if metadata.len() >= old.offset { old.offset } else { 0 };
            if reader.seek(SeekFrom::Start(offset)).is_err() {
                parse_errors += 1;
                continue;
            }
            let mut line = String::new();
            loop {
                let Ok(read) = reader.read_line(&mut line) else {
                    parse_errors += 1;
                    break;
                };
                if read == 0 {
                    break;
                }
                if !line.ends_with('\n') {
                    break;
                }
                offset += read as u64;
                match serde_json::from_str::<Value>(&line) {
                    Ok(value) => {
                        let Some(message) = value.get("message") else {
                            line.clear();
                            continue;
                        };
                        let usage = &message["usage"];
                        let Some(input) = usage.get("input_tokens").and_then(Value::as_i64) else {
                            line.clear();
                            continue;
                        };
                        let output = usage.get("output_tokens").and_then(Value::as_i64).unwrap_or_default();
                        let cache_read = usage.get("cache_read_input_tokens").and_then(Value::as_i64).unwrap_or_default();
                        let cache_write = usage.get("cache_creation_input_tokens").and_then(Value::as_i64).unwrap_or_default();
                        let timestamp = super::parse_timestamp(value.get("timestamp").and_then(Value::as_str));
                        let Some(timestamp) = timestamp else {
                            line.clear();
                            continue;
                        };
                        if timestamp >= request.start_time_unix && timestamp < request.end_time_unix {
                            let message_id = message.get("id").and_then(Value::as_str).unwrap_or("unknown");
                            let model = message.get("model").and_then(Value::as_str).map(str::to_string);
                            buckets.push(UsageBucket {
                                id: bucket_id(source_id, &format!("{}:{message_id}", super::file_key(path))),
                                source_id: source_id.to_string(),
                                usage_kind: "subscription_local".to_string(),
                                period_start: timestamp,
                                period_end: timestamp + 1,
                                model,
                                session_key: value.get("sessionId").and_then(Value::as_str).map(str::to_string),
                                input_tokens: Some(input),
                                output_tokens: Some(output),
                                cached_input_tokens: Some(cache_read + cache_write),
                                total_tokens: Some(input + output + cache_read + cache_write),
                                amount: None,
                                currency: None,
                                confidence: "observed".to_string(),
                                observed_at: super::now(),
                            });
                        }
                    }
                    Err(_) => parse_errors += 1,
                }
                line.clear();
            }

            let id = old.session_id.unwrap_or_else(|| super::file_key(path));
            update_file_progress(
                &mut checkpoint,
                path,
                super::FileProgress {
                    offset,
                    session_id: Some(id),
                    session_start: old.session_start.or_else(|| metadata
                        .modified()
                        .ok()
                        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|duration| duration.as_secs() as i64)),
                },
            );
        }

        let status = if parse_errors > 0 {
            source_status(
                source_id,
                "partial",
                Some(format!("{} linhas não puderam ser interpretadas.", parse_errors)),
            )
        } else {
            source_status(source_id, "connected", None)
        };
        result_with_checkpoint(app, source_id, buckets, status, checkpoint)
    }
}
