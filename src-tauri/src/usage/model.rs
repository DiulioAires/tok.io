use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageBucket {
    pub id: String,
    pub source_id: String,
    pub usage_kind: String,
    pub period_start: i64,
    pub period_end: i64,
    pub model: Option<String>,
    pub session_key: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cached_input_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub amount: Option<f64>,
    pub currency: Option<String>,
    pub confidence: String,
    pub observed_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceStatus {
    pub source_id: String,
    pub state: String,
    pub last_successful_refresh: Option<i64>,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageDashboard {
    pub generated_at: i64,
    pub buckets: Vec<UsageBucket>,
    pub sources: Vec<SourceStatus>,
}

pub struct RefreshRequest {
    pub start_time_unix: i64,
    pub end_time_unix: i64,
}

pub struct CollectorResult {
    pub buckets: Vec<UsageBucket>,
    pub status: SourceStatus,
}
