use chrono::{DateTime, Datelike, Utc};

use crate::error::DeepSeekError;
use crate::types::*;

// ── Public API ────────────────────────────────────────────────

/// Aggregate raw API responses into display-ready data using the new
/// by_api_key endpoint format.
pub fn aggregate(
    summary: UserSummaryData,
    amount_response: &ByApiKeyAmountResponse,
    cost_response: &ByApiKeyCostResponse,
    now: DateTime<Utc>,
) -> AggregatedData {
    let currency = resolve_currency(&summary);
    let balance = compute_balance(&summary, &currency);

    // Flatten cost and amount data into per-day, per-model entries
    let (daily_items, model_summaries) =
        flatten_responses(cost_response, amount_response, &currency);

    let period_cost: f64 = daily_items.iter().map(|d| d.cost).sum();
    let period_cost = if period_cost == 0.0 { 0.0 } else { period_cost }; // normalize -0.0
    let period_api_requests: u64 = daily_items.iter().map(|d| d.api_requests).sum();
    let period_tokens: u64 = daily_items.iter().map(|d| d.tokens).sum();
    let period_output_tokens: u64 = daily_items.iter().map(|d| d.output_tokens).sum();
    let period_cache_hit: u64 = daily_items.iter().map(|d| d.cache_hit).sum();
    let period_cache_miss: u64 = daily_items.iter().map(|d| d.cache_miss).sum();
    let cache_hit_rate = if period_cache_hit + period_cache_miss > 0 {
        period_cache_hit as f64 / (period_cache_hit + period_cache_miss) as f64
    } else {
        0.0
    };

    AggregatedData {
        balance,
        currency,
        period_cost,
        period_api_requests,
        period_tokens,
        period_output_tokens,
        period_cache_hit,
        period_cache_miss,
        cache_hit_rate,
        models: model_summaries,
        daily_items,
        last_updated: now,
    }
}

// ── Currency & Balance ────────────────────────────────────────

fn resolve_currency(summary: &UserSummaryData) -> String {
    if let Some(ref c) = summary.currency
        && !c.is_empty()
    {
        return c.clone();
    }
    for wallet in summary
        .normal_wallets
        .iter()
        .flatten()
        .chain(summary.bonus_wallets.iter().flatten())
    {
        if !wallet.currency.is_empty() {
            return wallet.currency.clone();
        }
    }
    "CNY".to_string()
}

fn compute_balance(summary: &UserSummaryData, currency: &str) -> f64 {
    if let Some(b) = summary.total_balance
        && b > 0.0
    {
        return b;
    }
    let topup = summary.topped_up_balance.unwrap_or(0.0);
    let granted = summary.granted_balance.unwrap_or(0.0);
    if topup + granted > 0.0 {
        return topup + granted;
    }
    sum_wallets(
        summary.normal_wallets.as_deref(),
        summary.bonus_wallets.as_deref(),
        currency,
    )
}

fn sum_wallets(
    normal: Option<&[WalletBalance]>,
    bonus: Option<&[WalletBalance]>,
    currency: &str,
) -> f64 {
    let mut total = 0.0;
    for wallet in normal
        .into_iter()
        .flatten()
        .chain(bonus.into_iter().flatten())
    {
        if wallet.currency == currency {
            total += wallet_balance_f64(&wallet.balance);
        }
    }
    total
}

fn wallet_balance_f64(v: &serde_json::Value) -> f64 {
    match v {
        serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0),
        serde_json::Value::String(s) => s.parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    }
}

// ── Response flattening ───────────────────────────────────────

/// Flatten the new by_api_key response format into per-day items and per-model summaries.
fn flatten_responses(
    cost: &ByApiKeyCostResponse,
    amount: &ByApiKeyAmountResponse,
    _currency: &str,
) -> (Vec<DailyItem>, Vec<ModelPeriodSummary>) {
    // Build a map: (date, model) -> (cost, api_requests, tokens)
    use std::collections::HashMap;

    #[derive(Default)]
    struct DayModelEntry {
        cost: f64,
        api_requests: u64,
        tokens: u64,
        output_tokens: u64,
        cache_hit: u64,
        cache_miss: u64,
    }

    let mut entries: HashMap<(String, String), DayModelEntry> = HashMap::new();
    let date_formatter = |ts: i64| {
        // Convert unix timestamp to YYYY-MM-DD in UTC
        let secs = ts;
        let days = secs / 86400;
        // 1970-01-01 is day 0
        let d = chrono::NaiveDate::from_ymd_opt(1970, 1, 1)
            .unwrap()
            .checked_add_signed(chrono::Duration::days(days))
            .unwrap();
        d.format("%Y-%m-%d").to_string()
    };

    // Flatten cost data: cost_response.data[].series[].buckets[]
    for group in &cost.data {
        for series in &group.series {
            let model = series.model.clone();
            for bucket in &series.buckets {
                let date = date_formatter(bucket.time);
                let cost_val = bucket.cost.parse::<f64>().unwrap_or(0.0);
                let entry = entries.entry((date.clone(), model.clone())).or_default();
                entry.cost += cost_val;
            }
        }
    }

    // Flatten amount data: amount_response.series[].buckets[]
    for series in &amount.series {
        let model = series.model.clone();
        for bucket in &series.buckets {
            let date = date_formatter(bucket.time);
            let entry = entries.entry((date.clone(), model.clone())).or_default();
            entry.api_requests += bucket.usage.request;
            entry.tokens += bucket.usage.total();
            entry.output_tokens += bucket.usage.response_token;
            entry.cache_hit += bucket.usage.prompt_cache_hit_token;
            entry.cache_miss += bucket.usage.prompt_cache_miss_token;
        }
    }

    // Build daily items sorted by date
    let mut daily_map: HashMap<String, DailyItem> = HashMap::new();
    for ((date, model), entry) in entries {
        let day = daily_map.entry(date.clone()).or_insert_with(|| DailyItem {
            date,
            cost: 0.0,
            api_requests: 0,
            tokens: 0,
            output_tokens: 0,
            cache_hit: 0,
            cache_miss: 0,
            models: Vec::new(),
        });
        day.cost += entry.cost;
        day.api_requests += entry.api_requests;
        day.tokens += entry.tokens;
        day.output_tokens += entry.output_tokens;
        day.cache_hit += entry.cache_hit;
        day.cache_miss += entry.cache_miss;
        day.models.push(ModelPeriodSummary {
            name: model,
            cost: entry.cost,
            api_requests: entry.api_requests,
            tokens: entry.tokens,
            output_tokens: entry.output_tokens,
            cache_hit: entry.cache_hit,
            cache_miss: entry.cache_miss,
        });
    }

    let mut daily_items: Vec<DailyItem> = daily_map.into_values().collect();
    daily_items.sort_by(|a, b| a.date.cmp(&b.date));

    // Build per-model summaries
    let mut model_map: HashMap<String, ModelPeriodSummary> = HashMap::new();
    for day in &daily_items {
        for m in &day.models {
            let entry = model_map
                .entry(m.name.clone())
                .or_insert(ModelPeriodSummary {
                    name: m.name.clone(),
                    cost: 0.0,
                    api_requests: 0,
                    tokens: 0,
                    output_tokens: 0,
                    cache_hit: 0,
                    cache_miss: 0,
                });
            entry.cost += m.cost;
            entry.api_requests += m.api_requests;
            entry.tokens += m.tokens;
            entry.output_tokens += m.output_tokens;
            entry.cache_hit += m.cache_hit;
            entry.cache_miss += m.cache_miss;
        }
    }
    let mut model_summaries: Vec<ModelPeriodSummary> = model_map.into_values().collect();
    model_summaries.sort_by(|a, b| {
        b.cost
            .partial_cmp(&a.cost)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    (daily_items, model_summaries)
}

// ── Utility helpers ──────────────────────────────────────────

pub fn format_cost(n: f64, currency: &str) -> String {
    let symbol = if currency == "CNY" { "¥" } else { "$" };
    let n = if n == 0.0 { 0.0 } else { n }; // normalize -0.0 → 0.0
    format!("{}{:.2}", symbol, n)
}

pub fn format_tokens(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.2}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

pub fn format_number(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    } else {
        n.to_string()
    }
}

// ── Time range helpers ────────────────────────────────────────

/// Compute start/end Unix timestamps for a given period option.
/// Start is midnight UTC of the first day, end is midnight UTC of the day after the last day.
pub fn compute_time_range(period: &str) -> (i64, i64) {
    let now = Utc::now();
    let today = now.date_naive();

    match period {
        "today" => {
            let start = today;
            let end = today + chrono::Duration::days(1);
            (
                start.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp(),
                end.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp(),
            )
        }
        "7d" => {
            let start = today - chrono::Duration::days(6);
            let end = today + chrono::Duration::days(1);
            (
                start.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp(),
                end.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp(),
            )
        }
        "30d" => {
            let start = today - chrono::Duration::days(29);
            let end = today + chrono::Duration::days(1);
            (
                start.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp(),
                end.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp(),
            )
        }
        "this-month" => {
            let start = today.with_day(1).unwrap();
            let end = today + chrono::Duration::days(1);
            (
                start.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp(),
                end.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp(),
            )
        }
        "last-month" => {
            let first_this_month = today.with_day(1).unwrap();
            let last_month_end = first_this_month - chrono::Duration::days(1);
            let last_month_start = last_month_end.with_day(1).unwrap();
            let next_month_start = first_this_month; // first day of this month = day after last month
            (
                last_month_start
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
                    .and_utc()
                    .timestamp(),
                next_month_start
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
                    .and_utc()
                    .timestamp(),
            )
        }
        _ => {
            // Default: last 7 days
            let start = today - chrono::Duration::days(6);
            let end = today + chrono::Duration::days(1);
            (
                start.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp(),
                end.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp(),
            )
        }
    }
}

/// Compute start/end Unix timestamps from explicit YYYY-MM-DD dates.
/// End is exclusive (midnight of the day after the selected end date).
pub fn compute_time_range_from_dates(
    start_str: &str,
    end_str: &str,
) -> Result<(i64, i64), DeepSeekError> {
    let start = chrono::NaiveDate::parse_from_str(start_str, "%Y-%m-%d")
        .map_err(|e| DeepSeekError::Parse(format!("invalid start date '{}': {}", start_str, e)))?;
    let end = chrono::NaiveDate::parse_from_str(end_str, "%Y-%m-%d")
        .map_err(|e| DeepSeekError::Parse(format!("invalid end date '{}': {}", end_str, e)))?;

    if end < start {
        return Err(DeepSeekError::Parse(format!(
            "end date {} is before start date {}",
            end_str, start_str
        )));
    }

    let start_ts = start.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
    let end_exclusive = end + chrono::Duration::days(1);
    let end_ts = end_exclusive
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp();

    Ok((start_ts, end_ts))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_cost_cny() {
        assert_eq!(format_cost(5.76, "CNY"), "¥5.76");
        assert_eq!(format_cost(0.0, "CNY"), "¥0.00");
        assert_eq!(format_cost(100.0, "CNY"), "¥100.00");
        assert_eq!(format_cost(-0.0, "CNY"), "¥0.00"); // normalize -0.0
    }

    #[test]
    fn test_format_cost_usd() {
        assert_eq!(format_cost(5.76, "USD"), "$5.76");
    }

    #[test]
    fn test_format_tokens() {
        assert_eq!(format_tokens(0), "0");
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(1_500), "1.5K");
        assert_eq!(format_tokens(1_234_567), "1.23M");
        assert_eq!(format_tokens(1_234_567_890), "1.23B");
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(500), "500");
        assert_eq!(format_number(1_500), "1500");
        assert_eq!(format_number(2_782), "2782");
        assert_eq!(format_number(275_649_934), "275.65M");
    }

    #[test]
    fn test_compute_time_range_today() {
        let (start, end) = compute_time_range("today");
        let today = Utc::now().date_naive();
        let expected_start = today.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
        let expected_end = (today + chrono::Duration::days(1))
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp();
        assert_eq!(start, expected_start);
        assert_eq!(end, expected_end);
        assert!((end - start - 86400).abs() < 2);
    }

    #[test]
    fn test_compute_time_range_7d() {
        let (start, end) = compute_time_range("7d");
        // End should be midnight of next day (up to 24h ahead of now)
        assert!(end >= Utc::now().timestamp());
        assert!(end - Utc::now().timestamp() <= 86400 + 5);
        // Range should be 7 days
        assert!((end - start - 7 * 86400).abs() < 2);
    }

    #[test]
    fn test_compute_time_range_30d() {
        let (start, end) = compute_time_range("30d");
        assert!((end - start - 30 * 86400).abs() < 2);
    }

    #[test]
    fn test_compute_time_range_default() {
        let (start, end) = compute_time_range("unknown");
        // Default should be 7d
        assert!((end - start - 7 * 86400).abs() < 2);
    }

    #[test]
    fn test_aggregate_basic() {
        let summary = UserSummaryData {
            total_balance: Some(32.72),
            currency: Some("CNY".into()),
            ..Default::default()
        };

        let amount = ByApiKeyAmountResponse {
            start: 0,
            end: 0,
            bucket: 86400,
            models: vec!["deepseek-v4-pro".into()],
            series: vec![AmountSeriesItem {
                api_key: ApiKeyMeta {
                    tracking_id: "test".into(),
                    name: "test-key".into(),
                    sensitive_id: "sk-test".into(),
                    valid: true,
                },
                model: "deepseek-v4-pro".into(),
                buckets: vec![AmountBucket {
                    time: 0, // 1970-01-01
                    usage: UsageMetrics {
                        response_token: 1000,
                        request: 5,
                        prompt_cache_hit_token: 5000,
                        prompt_cache_miss_token: 500,
                    },
                }],
            }],
        };

        let cost = ByApiKeyCostResponse {
            start: 0,
            end: 0,
            bucket: 86400,
            models: vec!["deepseek-v4-pro".into()],
            data: vec![CostCurrencyGroup {
                currency: "CNY".into(),
                series: vec![CostSeriesItem {
                    api_key: ApiKeyMeta {
                        tracking_id: "test".into(),
                        name: "test-key".into(),
                        sensitive_id: "sk-test".into(),
                        valid: true,
                    },
                    model: "deepseek-v4-pro".into(),
                    buckets: vec![NewCostBucket {
                        time: 0,
                        cost: "0.00".into(),
                    }],
                }],
            }],
        };

        let result = aggregate(summary, &amount, &cost, Utc::now());
        assert_eq!(result.balance, 32.72);
        assert_eq!(result.currency, "CNY");
        assert_eq!(result.period_cost, 0.0); // not -0.0
        assert_eq!(result.period_api_requests, 5);
        assert_eq!(result.period_tokens, 6500);
        assert_eq!(result.period_output_tokens, 1000);
        assert_eq!(result.period_cache_hit, 5000);
        assert_eq!(result.period_cache_miss, 500);
        assert!((result.cache_hit_rate - 5000.0 / 5500.0).abs() < 1e-6);
        assert_eq!(result.models.len(), 1);
        assert_eq!(result.models[0].name, "deepseek-v4-pro");
        assert_eq!(result.models[0].cost, 0.0);
        assert_eq!(result.models[0].api_requests, 5);
        assert_eq!(result.models[0].tokens, 6500);
        assert_eq!(result.models[0].output_tokens, 1000);
        assert_eq!(result.models[0].cache_hit, 5000);
        assert_eq!(result.models[0].cache_miss, 500);
    }
}
