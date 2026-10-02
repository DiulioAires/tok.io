use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde_json::Value;
use tauri::{AppHandle, Emitter};

#[derive(Clone, Copy)]
enum Provider {
    Codex,
    Claude,
}

impl Provider {
    fn label(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
        }
    }
}

pub struct CompletionWatcher {
    roots: Vec<(PathBuf, Provider)>,
    offsets: HashMap<PathBuf, u64>,
    claude_messages: HashSet<(PathBuf, String)>,
}

impl CompletionWatcher {
    pub fn new(roots: &[PathBuf]) -> Self {
        let roots = roots
            .iter()
            .filter_map(|root| {
                let provider = match root.file_name()?.to_str()? {
                    "sessions" => Provider::Codex,
                    "projects" => Provider::Claude,
                    _ => return None,
                };
                Some((root.clone(), provider))
            })
            .collect::<Vec<_>>();
        let mut offsets = HashMap::new();
        for (root, _) in &roots {
            seed_offsets(root, 0, &mut offsets);
        }
        Self {
            roots,
            offsets,
            claude_messages: HashSet::new(),
        }
    }

    pub fn scan_paths(&mut self, app: &AppHandle, paths: &[PathBuf]) {
        for path in paths {
            if path
                .extension()
                .is_none_or(|extension| extension != "jsonl")
            {
                continue;
            }
            let Some(provider) = self
                .roots
                .iter()
                .find(|(root, _)| path.starts_with(root))
                .map(|(_, provider)| *provider)
            else {
                continue;
            };
            self.scan_file(app, path, provider);
        }
    }

    fn scan_file(&mut self, app: &AppHandle, path: &Path, provider: Provider) {
        let Ok(file) = File::open(path) else { return };
        let Ok(length) = file.metadata().map(|metadata| metadata.len()) else {
            return;
        };
        let mut offset = self.offsets.get(path).copied().unwrap_or_default();
        if length < offset {
            offset = 0;
        }
        if length == offset {
            return;
        }
        let mut reader = BufReader::new(file);
        if reader.seek(SeekFrom::Start(offset)).is_err() {
            return;
        }
        let mut line = String::new();
        loop {
            let Ok(read) = reader.read_line(&mut line) else {
                break;
            };
            if read == 0 || !line.ends_with('\n') {
                break;
            }
            offset += read as u64;
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                let completed = match provider {
                    Provider::Codex => {
                        value["type"] == "event_msg" && value["payload"]["type"] == "task_complete"
                    }
                    Provider::Claude => {
                        if value["type"] != "assistant"
                            || value["isSidechain"] == true
                            || value["message"]["stop_reason"] != "end_turn"
                        {
                            false
                        } else if let Some(id) = value["message"]["id"].as_str() {
                            self.claude_messages
                                .insert((path.to_path_buf(), id.to_string()))
                        } else {
                            true
                        }
                    }
                };
                if completed {
                    let _ = app.emit("assistant-task-completed", provider.label());
                }
            }
            line.clear();
        }
        self.offsets.insert(path.to_path_buf(), offset);
    }
}

fn seed_offsets(root: &Path, depth: usize, offsets: &mut HashMap<PathBuf, u64>) {
    if depth > 8 {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            seed_offsets(&path, depth + 1, offsets);
        } else if kind.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension == "jsonl")
        {
            if let Ok(metadata) = entry.metadata() {
                offsets.insert(path, metadata.len());
            }
        }
    }
}
