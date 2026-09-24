use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;

use super::{claude_quota, codex_quota};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LockScreenSnapshot {
    updated_at: u64,
    codex: Option<codex_quota::CodexQuota>,
    claude: Option<claude_quota::ClaudeQuota>,
}

fn snapshot_path() -> Option<PathBuf> {
    let user_profile = std::env::var_os("USERPROFILE")?;
    Some(PathBuf::from(user_profile).join(".ai-usage-widget").join("lock-screen.json"))
}

fn publish() {
    let Some(path) = snapshot_path() else { return; };
    let snapshot = LockScreenSnapshot {
        updated_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
        codex: codex_quota::read().ok(),
        claude: claude_quota::read().ok(),
    };
    let Ok(json) = serde_json::to_vec(&snapshot) else { return; };
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_err() { return; }
    }
    let temporary = path.with_extension("json.tmp");
    if std::fs::write(&temporary, json).is_ok() {
        let _ = std::fs::rename(&temporary, &path);
    }
}

pub fn start() {
    std::thread::spawn(|| loop {
        publish();
        std::thread::sleep(Duration::from_secs(60));
    });
}
