use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Aggregated output ────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct AggregatedData {
    pub balance: f64,
    pub currency: String,
    pub period_cost: f64,
    pub period_api_requests: u64,
    pub period_tokens: u64,
    pub period_cache_hit: u64,
    pub period_cache_miss: u64,
    pub period_output_tokens: u64,
    pub cache_hit_rate: f64,
    pub models: Vec<ModelPeriodSummary>,
    pub daily_items: Vec<DailyItem>,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelPeriodSummary {
    pub name: String,
    pub cost: f64,
    pub api_requests: u64,
    pub tokens: u64,
    pub output_tokens: u64,
    pub cache_hit: u64,
    pub cache_miss: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DailyItem {
    pub date: String,
    pub cost: f64,
    pub api_requests: u64,
    pub tokens: u64,
    pub output_tokens: u64,
    pub cache_hit: u64,
    pub cache_miss: u64,
    pub models: Vec<ModelPeriodSummary>,
}

// ── API envelope ─────────────────────────────────────────────

#[derive(Deserialize, Debug)]
pub struct ApiEnvelope<T> {
    #[serde(default)]
    pub code: i32,
    #[serde(default)]
    pub msg: String,
    pub data: Option<T>,
}

#[derive(Deserialize, Debug)]
pub struct BizWrapper<T> {
    #[serde(default)]
    pub biz_code: i32,
    #[serde(default)]
    pub biz_msg: String,
    pub biz_data: T,
}

// ── User summary ─────────────────────────────────────────────

/// A value that may come back from DeepSeek as either a plain number
/// (f64) or a stringified number — accept both during deserialization,
/// store the canonical form as a String.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(untagged)]
enum StringOrF64Raw {
    Num(f64),
    Str(String),
    #[default]
    None,
}

/// String-or-f64 wrapper: deserialises from both JSON types, exposes the
/// value as `Option<String>` for callers.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)] // field 0 is read in the Deserialize impl; rustc Lint doesn't track that
pub struct StringOrF64(Option<String>);

impl<'de> Deserialize<'de> for StringOrF64 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = StringOrF64Raw::deserialize(deserializer)?;
        let s = match raw {
            StringOrF64Raw::Num(n) => Some(n.to_string()),
            StringOrF64Raw::Str(s) => Some(s),
            StringOrF64Raw::None => None,
        };
        Ok(Self(s))
    }
}

#[allow(dead_code)]
impl StringOrF64 {
    fn as_str(&self) -> Option<&str> {
        self.0.as_deref()
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct UserSummaryData {
    #[serde(default)]
    pub total_balance: Option<f64>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub month_cost: Option<f64>,
    #[serde(default)]
    pub total_usage: Option<f64>,
    #[serde(default)]
    pub topped_up_balance: Option<f64>,
    #[serde(default)]
    pub granted_balance: Option<f64>,
    #[serde(default)]
    pub current_token: Option<f64>,
    #[serde(default)]
    pub normal_wallets: Option<Vec<WalletBalance>>,
    #[serde(default)]
    pub bonus_wallets: Option<Vec<WalletBalance>>,
    #[serde(default)]
    pub monthly_costs: Option<Vec<MonthlyCost>>,
    #[serde(default)]
    pub total_available_token_estimation: Option<String>,
    #[serde(default)]
    pub monthly_token_usage: Option<StringOrF64>,
    #[serde(default)]
    pub monthly_usage: Option<StringOrF64>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct WalletBalance {
    pub currency: String,
    #[serde(default)]
    pub balance: serde_json::Value,
    pub token_estimation: Option<serde_json::Value>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MonthlyCost {
    pub currency: String,
    #[serde(default)]
    pub amount: serde_json::Value,
}

// ── Cost endpoint ────────────────────────────────────────────

/// Schema A (flat)
#[derive(Deserialize, Debug, Clone)]
pub struct CostFlat {
    pub items: Vec<CostFlatItem>,
    #[serde(default)]
    pub month: i32,
    #[serde(default)]
    pub year: i32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CostFlatItem {
    pub date: String,
    #[serde(default)]
    pub models: Vec<ModelCostPayload>,
    #[serde(default)]
    pub total: f64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModelCostPayload {
    pub name: String,
    #[serde(default)]
    pub cost: f64,
}

/// Schema B (bucketed)
pub type CostBucketed = Vec<CostBucket>;

#[derive(Deserialize, Debug, Clone)]
pub struct CostBucket {
    pub currency: String,
    #[serde(default)]
    pub total: Vec<ModelUsageEntry>,
    #[serde(default)]
    pub days: Vec<UsageDay>,
}

// ── Amount endpoint ──────────────────────────────────────────

/// Schema A (flat)
#[derive(Deserialize, Debug, Clone)]
pub struct AmountFlat {
    pub items: Vec<AmountFlatItem>,
    #[serde(default)]
    pub month: i32,
    #[serde(default)]
    pub year: i32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AmountFlatItem {
    pub date: String,
    #[serde(default)]
    pub models: Vec<ModelAmountPayload>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModelAmountPayload {
    pub name: String,
    #[serde(default)]
    pub input_cache_hit: u64,
    #[serde(default)]
    pub input_cache_miss: u64,
    #[serde(default)]
    pub output: u64,
    #[serde(default)]
    pub api_requests: u64,
}

/// Schema B (bucketed)
#[derive(Deserialize, Debug, Clone)]
pub struct AmountBucketed {
    #[serde(default)]
    pub total: Vec<ModelUsageEntry>,
    #[serde(default)]
    pub days: Vec<UsageDay>,
}

// ── Shared (bucket schema) ───────────────────────────────────

#[derive(Deserialize, Debug, Clone)]
pub struct UsageDay {
    pub date: String,
    #[serde(default)]
    pub data: Vec<ModelUsageEntry>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModelUsageEntry {
    pub model: String,
    #[serde(default)]
    pub usage: Vec<UsageMetric>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct UsageMetric {
    #[serde(rename = "type")]
    pub metric_type: String,
    #[serde(default)]
    pub amount: serde_json::Value,
}

// ── New: by_api_key endpoints ──────────────────────────────────

/// Response from GET /api/v0/users/get_api_keys
#[derive(Deserialize, Debug, Clone)]
pub struct ApiKeysResponse {
    pub api_keys: Vec<ApiKeyInfo>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ApiKeyInfo {
    pub tracking_id: String,
    pub name: String,
    pub sensitive_id: String,
    pub created_at: i64,
    pub last_use: i64,
}

/// Meta info embedded in each series item
#[derive(Deserialize, Debug, Clone)]
pub struct ApiKeyMeta {
    pub tracking_id: String,
    pub name: String,
    pub sensitive_id: String,
    pub valid: bool,
}

/// Flat usage metrics object (new format: keys are metric type names)
#[derive(Debug, Clone, Default)]
pub struct UsageMetrics {
    pub response_token: u64,
    pub request: u64,
    pub prompt_cache_hit_token: u64,
    pub prompt_cache_miss_token: u64,
}

impl UsageMetrics {
    pub fn total(&self) -> u64 {
        self.response_token + self.prompt_cache_hit_token + self.prompt_cache_miss_token
    }
}

impl<'de> Deserialize<'de> for UsageMetrics {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw: std::collections::HashMap<String, serde_json::Value> =
            std::collections::HashMap::deserialize(deserializer)?;
        let to_u64 = |v: &serde_json::Value| match v {
            serde_json::Value::Number(n) => n.as_u64().unwrap_or(0),
            serde_json::Value::String(s) => s.parse::<u64>().unwrap_or(0),
            _ => 0,
        };
        Ok(UsageMetrics {
            response_token: to_u64(
                raw.get("RESPONSE_TOKEN")
                    .unwrap_or(&serde_json::Value::Null),
            ),
            request: to_u64(raw.get("REQUEST").unwrap_or(&serde_json::Value::Null)),
            prompt_cache_hit_token: to_u64(
                raw.get("PROMPT_CACHE_HIT_TOKEN")
                    .unwrap_or(&serde_json::Value::Null),
            ),
            prompt_cache_miss_token: to_u64(
                raw.get("PROMPT_CACHE_MISS_TOKEN")
                    .unwrap_or(&serde_json::Value::Null),
            ),
        })
    }
}

/// Response from GET /api/v0/usage/by_api_key/amount
#[derive(Deserialize, Debug, Clone)]
pub struct ByApiKeyAmountResponse {
    pub start: i64,
    pub end: i64,
    pub bucket: i64,
    pub models: Vec<String>,
    pub series: Vec<AmountSeriesItem>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AmountSeriesItem {
    #[serde(rename = "api_key")]
    pub api_key: ApiKeyMeta,
    pub model: String,
    pub buckets: Vec<AmountBucket>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AmountBucket {
    pub time: i64,
    pub usage: UsageMetrics,
}

/// Response from GET /api/v0/usage/by_api_key/cost
#[derive(Deserialize, Debug, Clone)]
pub struct ByApiKeyCostResponse {
    pub start: i64,
    pub end: i64,
    pub bucket: i64,
    pub models: Vec<String>,
    pub data: Vec<CostCurrencyGroup>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CostCurrencyGroup {
    pub currency: String,
    pub series: Vec<CostSeriesItem>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CostSeriesItem {
    #[serde(rename = "api_key")]
    pub api_key: ApiKeyMeta,
    pub model: String,
    pub buckets: Vec<NewCostBucket>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct NewCostBucket {
    pub time: i64,
    #[serde(default)]
    pub cost: String,
}

// ── Response enums (polymorphic deserialization) ─────────────

pub enum CostResponse {
    Flat(CostFlat),
    Bucketed(CostBucketed),
}

pub enum AmountResponse {
    Flat(AmountFlat),
    Bucketed(AmountBucketed),
}
