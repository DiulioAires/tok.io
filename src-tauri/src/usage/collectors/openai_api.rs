use reqwest::blocking::Client;
use serde_json::Value;
use tauri::AppHandle;

use super::super::credentials;
use super::super::model::{CollectorResult, RefreshRequest, SourceStatus, UsageBucket};
use super::now;

const SOURCE_ID: &str = "openai_api";

pub fn collect(app: &AppHandle, request: &RefreshRequest) -> CollectorResult {
    if !super::super::storage::source_enabled(app, SOURCE_ID).unwrap_or(true) {
        return result(Vec::new(), "unavailable", Some("Coleta desativada nas configurações.".into()));
    }
    let key = match credentials::get("openai") {
        Ok(key) => key,
        Err(_) => return result(Vec::new(), "unavailable", Some("Configure uma chave administrativa de organização para consultar uso e custos da API.".into())),
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
    let mut page: Option<String> = None;
    let mut buckets = Vec::new();
    loop {
        let mut query = vec![
            ("start_time", request.start_time_unix.to_string()),
            ("end_time", request.end_time_unix.to_string()),
            ("bucket_width", "1d".to_string()),
            ("limit", "31".to_string()),
            ("group_by[]", "model".to_string()),
        ];
        if let Some(cursor) = page.as_ref() {
            query.push(("page", cursor.clone()));
        }
        let response = match client
            .get("https://api.openai.com/v1/organization/usage/completions")
            .bearer_auth(key)
            .query(&query)
            .send()
        {
            Ok(response) => response,
            Err(_) => return (buckets, false, Some("Falha de rede ao consultar tokens da API OpenAI.".into())),
        };
        if !response.status().is_success() {
            return (buckets, false, Some(openai_error(response.status().as_u16())));
        }
        let body: Value = match response.json() {
            Ok(body) => body,
            Err(_) => return (buckets, false, Some("Resposta inválida da API OpenAI.".into())),
        };
        for bucket in body.get("data").and_then(Value::as_array).into_iter().flatten() {
            let start = bucket.get("start_time").and_then(Value::as_i64).unwrap_or(request.start_time_unix);
            let end = bucket.get("end_time").and_then(Value::as_i64).unwrap_or(start + 86_400);
            for record in bucket.get("results").and_then(Value::as_array).into_iter().flatten() {
                let input = int_field(record, "input_tokens");
                let output = int_field(record, "output_tokens");
                let cached = int_field(record, "input_cached_tokens");
                let total = input.zip(output).map(|(input, output)| input + output);
                if input.is_none() && output.is_none() {
                    continue;
                }
                let model = record.get("model").and_then(Value::as_str).map(str::to_string);
                let row_key = format!("{start}:{}", model.as_deref().unwrap_or("all"));
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
                    cached_input_tokens: cached,
                    total_tokens: total,
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
            return (buckets, false, Some("A paginação da API OpenAI terminou inesperadamente.".into()));
        }
        page = next;
    }
}

fn fetch_costs(client: &Client, key: &str, request: &RefreshRequest) -> (Vec<UsageBucket>, bool, Option<String>) {
    let mut page: Option<String> = None;
    let mut buckets = Vec::new();
    loop {
        let mut query = vec![
            ("start_time", request.start_time_unix.to_string()),
            ("end_time", request.end_time_unix.to_string()),
            ("bucket_width", "1d".to_string()),
            ("limit", "31".to_string()),
        ];
        if let Some(cursor) = page.as_ref() {
            query.push(("page", cursor.clone()));
        }
        let response = match client
            .get("https://api.openai.com/v1/organization/costs")
            .bearer_auth(key)
            .query(&query)
            .send()
        {
            Ok(response) => response,
            Err(_) => return (buckets, false, Some("Falha de rede ao consultar custos da API OpenAI.".into())),
        };
        if !response.status().is_success() {
            return (buckets, false, Some(openai_error(response.status().as_u16())));
        }
        let body: Value = match response.json() {
            Ok(body) => body,
            Err(_) => return (buckets, false, Some("Resposta inválida da API OpenAI.".into())),
        };
        for bucket in body.get("data").and_then(Value::as_array).into_iter().flatten() {
            let start = bucket.get("start_time").and_then(Value::as_i64).unwrap_or(request.start_time_unix);
            let end = bucket.get("end_time").and_then(Value::as_i64).unwrap_or(start + 86_400);
            for record in bucket.get("results").and_then(Value::as_array).into_iter().flatten() {
                let amount = record.get("amount").and_then(|value| value.get("value")).and_then(Value::as_f64);
                let Some(amount) = amount else { continue };
                let currency = record.get("amount").and_then(|value| value.get("currency")).and_then(Value::as_str).map(str::to_uppercase);
                let group = record.get("line_item").and_then(Value::as_str).unwrap_or("all");
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
                    amount: Some(amount),
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
            return (buckets, false, Some("A paginação dos custos OpenAI terminou inesperadamente.".into()));
        }
        page = next;
    }
}

fn int_field(value: &Value, name: &str) -> Option<i64> {
    value.get(name).and_then(Value::as_i64)
}

fn openai_error(status: u16) -> String {
    match status {
        401 => "A chave OpenAI foi recusada. Confira se é uma chave administrativa válida.".into(),
        403 => "A chave OpenAI não tem permissão para consultar uso e custos da organização.".into(),
        429 => "A API OpenAI limitou as consultas. Os dados serão atualizados depois.".into(),
        _ => "A API OpenAI não conseguiu fornecer estes dados agora.".into(),
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
