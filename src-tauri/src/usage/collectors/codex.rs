use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;

use serde_json::Value;
use tauri::AppHandle;

use super::super::model::{CollectorResult, RefreshRequest, UsageBucket};
use super::{
    bucket_id, file_progress, read_checkpoint, result_with_checkpoint, source_status,
    update_file_progress, FileProgress, UsageCollector,
};

pub struct CodexCollector;

impl UsageCollector for CodexCollector {
    fn source_id(&self) -> &'static str {
        "codex_local"
    }

    fn discover(&self) -> Result<Vec<PathBuf>, String> {
        let Some(root) = super::resolve_root("CODEX_HOME", ".codex") else {
            return Err("Não foi possível localizar a pasta do usuário do Windows.".into());
        };
        let sessions = root.join("sessions");
        if !sessions.is_dir() {
            return Ok(Vec::new());
        }
        Ok(super::walk_jsonl(&sessions, 5)
            .into_iter()
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("rollout-"))
            })
            .collect())
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
            let old = file_progress(&checkpoint, path);
            let Ok(file) = std::fs::File::open(path) else {
                parse_errors += 1;
                continue;
            };
            let Ok(metadata) = file.metadata() else {
                parse_errors += 1;
                continue;
            };
            let mut offset = if metadata.len() >= old.offset {
                old.offset
            } else {
                0
            };
            let mut session_id = old.session_id;
            let mut session_start = old.session_start;
            let mut latest: Option<(String, i64, i64, i64, i64)> = None;
            let mut reader = BufReader::new(file);
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
                let parsed: Result<Value, _> = serde_json::from_str(&line);
                match parsed {
                    Ok(value) => {
                        let kind = value
                            .get("payload")
                            .and_then(|payload| payload.get("type"))
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        if kind == "session_meta" {
                            let payload = &value["payload"];
                            session_id = payload.get("id").and_then(Value::as_str).map(str::to_string);
                            session_start = super::parse_timestamp(
                                payload.get("timestamp").and_then(Value::as_str),
                            );
                        } else if kind == "token_count" {
                            let total = &value["payload"]["info"]["total_token_usage"];
                            let input = total.get("input_tokens").and_then(Value::as_i64);
                            let output = total.get("output_tokens").and_then(Value::as_i64);
                            let cached = total.get("cached_input_tokens").and_then(Value::as_i64);
                            let tokens = total.get("total_tokens").and_then(Value::as_i64);
                            let model = value["payload"]["info"]["model"]
                                .as_str()
                                .unwrap_or_default()
                                .to_string();
                            if let Some(tokens) = tokens.or_else(|| match (input, output) {
                                (Some(input), Some(output)) => Some(input + output),
                                _ => None,
                            }) {
                                latest = Some((model, input.unwrap_or_default(), output.unwrap_or_default(), cached.unwrap_or_default(), tokens));
                            }
                        }
                    }
                    Err(_) => parse_errors += 1,
                }
                line.clear();
            }

            let modified = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs() as i64)
                .unwrap_or_default();
            let start = session_start.unwrap_or(modified);
            let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or("session");
            let id = session_id.unwrap_or_else(|| file_name.to_string());
            if let Some((model, input, output, cached, total)) = latest {
                if start >= request.start_time_unix && start < request.end_time_unix {
                    buckets.push(UsageBucket {
                        id: bucket_id(source_id, &id),
                        source_id: source_id.to_string(),
                        usage_kind: "subscription_local".to_string(),
                        period_start: start,
                        period_end: start + 1,
                        model: (!model.is_empty()).then_some(model),
                        session_key: Some(id.clone()),
                        input_tokens: Some(input),
                        output_tokens: Some(output),
                        cached_input_tokens: Some(cached),
                        total_tokens: Some(total),
                        amount: None,
                        currency: None,
                        confidence: "observed".to_string(),
                        observed_at: super::now(),
                    });
                }
            }
            update_file_progress(
                &mut checkpoint,
                path,
                FileProgress {
                    offset,
                    session_id: Some(id),
                    session_start: Some(start),
                },
            );
        }

        let status = if parse_errors > 0 {
            source_status(
                source_id,
                "partial",
                Some(format!("{} sessões não puderam ser lidas por completo.", parse_errors)),
            )
        } else {
            source_status(source_id, "connected", None)
        };
        result_with_checkpoint(app, source_id, buckets, status, checkpoint)
    }
}
