use chrono::{DateTime, Utc};
use reqwest::blocking::Client;
use serde_json::Value;
use tauri::AppHandle;

use super::super::credentials;
use super::super::model::{CollectorResult, RefreshRequest, SourceStatus, UsageBucket};
use super::now;

const SOURCE_ID: &str = "anthropic_api";
const API_VERSION: &str = "2023-06-01";

pub fn collect(app: &AppHandle, request: &RefreshRequest) -> CollectorResult {
    if !super::super::storage::source_enabled(app, SOURCE_ID).unwrap_or(true) {
        return result(Vec::new(), "unavailable", Some("Coleta desativada nas configurações.".into()));
    }
    let key = match credentials::get("anthropic") {
        Ok(key) => key,
        Err(_) => return result(Vec::new(), "unavailable", Some("Configure uma chave Admin API da organização Anthropic para consultar uso e custos.".into())),
    };
    let client = match Client::builder()
        .connect_timeout(std::time::Duration::from_secs(8))
        .timeout(std::time::Duration::from_secs(20))
        .build()
    {
        Ok(client) => client,
        Err(_) => return result(Vec::new(), "unavailable", Some("Não foi possível preparar a conexão com a API.".into())),
    };

    let (usage_buckets, usage_ok, usage_error) = fetch_usage(&client, &key, request);
    let (cost_buckets, costs_ok, costs_error) = fetch_costs(&client, &key, request);
    let mut buckets = usage_buckets;
    buckets.extend(cost_buckets);
    let (state, message) = match (usage_ok, costs_ok) {
        (true, true) => ("connected", None),
        (false, false) => ("stale", Some(usage_error.or(costs_error).unwrap_or_else(|| "A API não retornou dados.".into()))),
        _ => ("partial", Some(usage_error.or(costs_error).unwrap_or_else(|| "Alguns dados da API não puderam ser carregados.".into()))),
    };
    result(buckets, state, message)
}

fn fetch_usage(client: &Client, key: &str, request: &RefreshRequest) -> (Vec<UsageBucket>, bool, Option<String>) {
    let starting_at = DateTime::<Utc>::from_timestamp(request.start_time_unix, 0)
        .unwrap_or_else(Utc::now)
        .to_rfc3339();
    let ending_at = DateTime::<Utc>::from_timestamp(request.end_time_unix, 0)
        .unwrap_or_else(Utc::now)
        .to_rfc3339();
    let mut page: Option<String> = None;
    let mut buckets = Vec::new();
    loop {
        let mut query = vec![
            ("starting_at", starting_at.clone()),
            ("ending_at", ending_at.clone()),
            ("bucket_width", "1d".to_string()),
            ("limit", "31".to_string()),
            ("group_by[]", "model".to_string()),
        ];
        if let Some(cursor) = page.as_ref() {
            query.push(("page", cursor.clone()));
        }
        let response = match client
            .get("https://api.anthropic.com/v1/organizations/usage_report/messages")
            .header("x-api-key", key)
            .header("anthropic-version", API_VERSION)
            .query(&query)
            .send()
        {
            Ok(response) => response,
            Err(_) => return (buckets, false, Some("Falha de rede ao consultar tokens da API Anthropic.".into())),
        };
        if !response.status().is_success() {
            return (buckets, false, Some(anthropic_error(response.status().as_u16())));
        }
        let body: Value = match response.json() {
            Ok(body) => body,
            Err(_) => return (buckets, false, Some("Resposta inválida da API Anthropic.".into())),
        };
        for bucket in body.get("data").and_then(Value::as_array).into_iter().flatten() {
            let start = timestamp_field(bucket, "starting_at").unwrap_or(request.start_time_unix);
            let end = timestamp_field(bucket, "ending_at").unwrap_or(start + 86_400);
            for record in bucket.get("results").and_then(Value::as_array).into_iter().flatten() {
                let input = int_field(record, "uncached_input_tokens");
                let output = int_field(record, "output_tokens");
                let cache_read = int_field(record, "cache_read_input_tokens").unwrap_or_default();
                let cache_creation = record
                    .get("cache_creation")
                    .map(|cache| int_field(cache, "ephemeral_1h_input_tokens").unwrap_or_default()
                        + int_field(cache, "ephemeral_5m_input_tokens").unwrap_or_default())
                    .unwrap_or_default();
                if input.is_none() && output.is_none() && cache_read == 0 && cache_creation == 0 {
                    continue;
                }
                let model = record.get("model").and_then(Value::as_str).map(str::to_string);
                let row_key = format!("{start}:{}", model.as_deref().unwrap_or("all"));
                let total = input.unwrap_or_default() + output.unwrap_or_default() + cache_read + cache_creation;
                buckets.push(UsageBucket {
                    id: format!("{SOURCE_ID}:usage:{row_key}"),
                    source_id: SOURCE_ID.into(),
                    usage_kind: "api_usage".into(),
                    period_start: start,
                    period_end: end,
                    model,
                    session_key: None,
                    input_tokens: input,
                    output_tokens: output,
                    cached_input_tokens: Some(cache_read + cache_creation),
                    total_tokens: Some(total),
                    amount: None,
                    currency: None,
                    confidence: "official".into(),
                    observed_at: now(),
                });
            }
        }
        if !body.get("has_more").and_then(Value::as_bool).unwrap_or(false) {
            return (buckets, true, None);
        }
        let next = body.get("next_page").and_then(Value::as_str).map(str::to_string);
        if next.is_none() || next == page {
            return (buckets, false, Some("A paginação da API Anthropic terminou inesperadamente.".into()));
        }
        page = next;
    }
}

fn fetch_costs(client: &Client, key: &str, request: &RefreshRequest) -> (Vec<UsageBucket>, bool, Option<String>) {
    let starting_at = DateTime::<Utc>::from_timestamp(request.start_time_unix, 0)
        .unwrap_or_else(Utc::now)
        .to_rfc3339();
    let ending_at = DateTime::<Utc>::from_timestamp(request.end_time_unix, 0)
        .unwrap_or_else(Utc::now)
        .to_rfc3339();
    let mut page: Option<String> = None;
    let mut buckets = Vec::new();
    loop {
        let mut query = vec![
            ("starting_at", starting_at.clone()),
            ("ending_at", ending_at.clone()),
            ("bucket_width", "1d".to_string()),
            ("limit", "31".to_string()),
        ];
        if let Some(cursor) = page.as_ref() {
            query.push(("page", cursor.clone()));
        }
        let response = match client
            .get("https://api.anthropic.com/v1/organizations/cost_report")
            .header("x-api-key", key)
            .header("anthropic-version", API_VERSION)
            .query(&query)
            .send()
        {
            Ok(response) => response,
            Err(_) => return (buckets, false, Some("Falha de rede ao consultar custos da API Anthropic.".into())),
        };
        if !response.status().is_success() {
            return (buckets, false, Some(anthropic_error(response.status().as_u16())));
        }
        let body: Value = match response.json() {
            Ok(body) => body,
            Err(_) => return (buckets, false, Some("Resposta inválida da API Anthropic.".into())),
        };
        for bucket in body.get("data").and_then(Value::as_array).into_iter().flatten() {
            let start = timestamp_field(bucket, "starting_at").unwrap_or(request.start_time_unix);
            let end = timestamp_field(bucket, "ending_at").unwrap_or(start + 86_400);
            for record in bucket.get("results").and_then(Value::as_array).into_iter().flatten() {
                let amount_cents = record
                    .get("amount")
                    .and_then(|amount| amount.get("value"))
                    .and_then(|amount| amount.as_f64().or_else(|| amount.as_str()?.parse().ok()));
                let Some(amount_cents) = amount_cents else { continue };
                let currency = record
                    .get("amount")
                    .and_then(|amount| amount.get("currency"))
                    .and_then(Value::as_str)
                    .map(str::to_uppercase);
                let group = record.get("description").and_then(Value::as_str).unwrap_or("all");
                buckets.push(UsageBucket {
                    id: format!("{SOURCE_ID}:cost:{start}:{group}"),
                    source_id: SOURCE_ID.into(),
                    usage_kind: "api_cost".into(),
                    period_start: start,
                    period_end: end,
                    model: None,
                    session_key: None,
                    input_tokens: None,
                    output_tokens: None,
                    cached_input_tokens: None,
                    total_tokens: None,
                    amount: Some(amount_cents / 100.0),
                    currency,
                    confidence: "official".into(),
                    observed_at: now(),
                });
            }
        }
        if !body.get("has_more").and_then(Value::as_bool).unwrap_or(false) {
            return (buckets, true, None);
        }
        let next = body.get("next_page").and_then(Value::as_str).map(str::to_string);
        if next.is_none() || next == page {
            return (buckets, false, Some("A paginação dos custos Anthropic terminou inesperadamente.".into()));
        }
        page = next;
    }
}

fn timestamp_field(value: &Value, field: &str) -> Option<i64> {
    value
        .get(field)
        .and_then(Value::as_str)
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|timestamp| timestamp.timestamp())
}

fn int_field(value: &Value, name: &str) -> Option<i64> {
    value.get(name).and_then(Value::as_i64)
}

fn anthropic_error(status: u16) -> String {
    match status {
        401 => "A chave Anthropic foi recusada. Confira se é uma chave administrativa válida.".into(),
        403 => "Esta conta não tem acesso aos relatórios Admin API da Anthropic.".into(),
        429 => "A API Anthropic limitou as consultas. Os dados serão atualizados depois.".into(),
        _ => "A API Anthropic não conseguiu fornecer estes dados agora.".into(),
    }
}

fn result(buckets: Vec<UsageBucket>, state: &str, message: Option<String>) -> CollectorResult {
    CollectorResult {
        buckets,
        status: SourceStatus {
            source_id: SOURCE_ID.into(),
            state: state.into(),
            last_successful_refresh: if state == "connected" || state == "partial" { Some(now()) } else { None },
            message,
        },
    }
}
