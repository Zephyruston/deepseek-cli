use std::time::Duration;

use crate::Result;
use crate::constants::{API_SUCCESS_CODE, API_TIMEOUT_SECS, DEEPSEEK_API_BASE, TOKEN_EXPIRED_CODE};
use crate::error::DeepSeekError;
use crate::types::*;

pub struct ApiClient {
    agent: ureq::Agent,
}

impl Default for ApiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ApiClient {
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(API_TIMEOUT_SECS)))
            .https_only(true)
            .build();
        Self {
            agent: config.new_agent(),
        }
    }

    /// Set the Bearer token for all subsequent requests.
    /// The token is stored in the agent's default headers.
    pub fn set_token(&self, _token: &str) {
        // We pass token per-request because ureq v3 Agent is immutable
        // after creation. We store it externally and inject in request().
    }

    pub fn get_user_summary(&self, token: &str) -> Result<UserSummaryData> {
        self.request::<UserSummaryData>(token, "/users/get_user_summary")
    }

    pub fn get_usage_cost(&self, token: &str, month: i32, year: i32) -> Result<CostResponse> {
        let path = format!("/usage/cost?month={}&year={}", month, year);
        let data = self.request_json(token, &path)?;
        let data = try_unwrap_biz(data);
        parse_cost_response(data)
    }

    pub fn get_usage_amount(&self, token: &str, month: i32, year: i32) -> Result<AmountResponse> {
        let path = format!("/usage/amount?month={}&year={}", month, year);
        let data = self.request_json(token, &path)?;
        let data = try_unwrap_biz(data);
        parse_amount_response(data)
    }

    /// Generic request returning a deserialized T from the data field.
    fn request<T: serde::de::DeserializeOwned>(&self, token: &str, path: &str) -> Result<T> {
        let data = self.request_json(token, path)?;
        unwrap_biz_data(data)
    }

    /// Raw request returning the `data` field as serde_json::Value.
    fn request_json(&self, token: &str, path: &str) -> Result<serde_json::Value> {
        let url = format!("{}{}", DEEPSEEK_API_BASE, path);

        let resp = self
            .agent
            .get(&url)
            .header("Authorization", &format!("Bearer {}", token))
            .header("Accept", "application/json")
            .header("User-Agent", "deepseek-cli/0.1")
            .call()
            .map_err(DeepSeekError::from)?;

        let body = resp
            .into_body()
            .read_to_string()
            .map_err(|e| DeepSeekError::Parse(format!("failed to read response body: {}", e)))?;

        let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
            DeepSeekError::Parse(format!(
                "invalid JSON: {} — body: {}",
                e,
                truncate_str(&body, 200)
            ))
        })?;

        let code = json.get("code").and_then(|c| c.as_i64()).unwrap_or(-1) as i32;

        if code == TOKEN_EXPIRED_CODE {
            return Err(DeepSeekError::TokenExpired);
        }
        if code != API_SUCCESS_CODE {
            let msg = json
                .get("msg")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown")
                .to_string();
            return Err(DeepSeekError::ApiError { code, msg });
        }

        json.get("data")
            .cloned()
            .ok_or_else(|| DeepSeekError::Parse("response missing 'data' field".into()))
    }
}

/// If data has a `biz_data` field, extract it. Otherwise return data as-is.
/// Does NOT validate biz_code — just extracts the inner value.
fn try_unwrap_biz(data: serde_json::Value) -> serde_json::Value {
    if let Some(inner) = data.get("biz_data") {
        return inner.clone();
    }
    data
}

/// Unwrap biz_data wrapper if present, otherwise deserialize data directly.
fn unwrap_biz_data<T: serde::de::DeserializeOwned>(data: serde_json::Value) -> Result<T> {
    // Check for biz_data wrapper first (serde ignores unknown fields by default,
    // so direct deserialization of a biz-wrapped struct would succeed with all None values)
    if data.get("biz_data").is_some()
        && let Ok(biz) = serde_json::from_value::<BizWrapper<T>>(data.clone())
    {
        if biz.biz_code == 0 {
            return Ok(biz.biz_data);
        }
        return Err(DeepSeekError::ApiBizError {
            code: biz.biz_code,
            msg: biz.biz_msg,
        });
    }
    // Direct deserialization (no biz_data wrapper)
    serde_json::from_value::<T>(data)
        .map_err(|e| DeepSeekError::Parse(format!("failed to deserialize response: {}", e)))
}

fn parse_cost_response(data: serde_json::Value) -> Result<CostResponse> {
    // Try Schema A (flat items)
    if let Ok(flat) = serde_json::from_value::<CostFlat>(data.clone()) {
        return Ok(CostResponse::Flat(flat));
    }
    // Try Schema B (bucketed)
    if let Ok(bucketed) = serde_json::from_value::<CostBucketed>(data) {
        return Ok(CostResponse::Bucketed(bucketed));
    }
    Err(DeepSeekError::Parse("unknown cost response schema".into()))
}

fn parse_amount_response(data: serde_json::Value) -> Result<AmountResponse> {
    // Try Schema A (flat items)
    if let Ok(flat) = serde_json::from_value::<AmountFlat>(data.clone()) {
        return Ok(AmountResponse::Flat(flat));
    }
    // Try Schema B (bucketed)
    if let Ok(bucketed) = serde_json::from_value::<AmountBucketed>(data) {
        return Ok(AmountResponse::Bucketed(bucketed));
    }
    Err(DeepSeekError::Parse(
        "unknown amount response schema".into(),
    ))
}

fn truncate_str(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}
