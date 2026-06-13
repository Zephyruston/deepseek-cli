use chrono::{DateTime, Utc};

use crate::types::*;

/// Aggregate raw API responses into display-ready data.
pub fn aggregate(
    summary: UserSummaryData,
    cost_response: &CostResponse,
    amount_response: &AmountResponse,
    now: DateTime<Utc>,
) -> AggregatedData {
    let today_str = now.format("%Y-%m-%d").to_string();

    let currency = resolve_currency(&summary, cost_response);
    let balance = compute_balance(&summary, &currency);
    let monthly_cost = compute_monthly_cost(&summary, &currency, cost_response);

    let cost_items = normalize_cost_items(cost_response, &currency);
    let amount_items = normalize_amount_items(amount_response);

    let today_cost_item = cost_items.iter().find(|item| item.date == today_str);
    let today_amount_item = amount_items.iter().find(|item| item.date == today_str);

    let today_cost = today_cost_item.map(|i| i.total).unwrap_or(0.0);
    let today_cost_by_model = today_cost_item
        .map(|i| i.models.clone())
        .unwrap_or_default()
        .into_iter()
        .map(|m| ModelCostEntry {
            name: m.name,
            cost: m.cost,
        })
        .collect();

    let token_summary = compute_token_summary(today_amount_item);

    let today_api_requests = today_amount_item
        .map(|item| item.models.iter().map(|m| m.api_requests).sum())
        .unwrap_or(0);

    AggregatedData {
        balance,
        currency,
        monthly_cost,
        today_cost,
        today_cost_by_model,
        today_tokens: token_summary,
        today_api_requests,
        last_updated: now,
    }
}

// ── Currency resolution ─────────────────────────────────────

fn resolve_currency(summary: &UserSummaryData, cost_response: &CostResponse) -> String {
    if let Some(ref c) = summary.currency
        && !c.is_empty()
    {
        return c.clone();
    }
    // Try wallets
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
    // Try cost bucket
    if let CostResponse::Bucketed(buckets) = cost_response
        && let Some(bucket) = buckets.first()
        && !bucket.currency.is_empty()
    {
        return bucket.currency.clone();
    }
    "CNY".to_string()
}

// ── Balance computation ─────────────────────────────────────

fn compute_balance(summary: &UserSummaryData, currency: &str) -> f64 {
    // If total_balance is explicitly set and non-zero, use it
    if let Some(b) = summary.total_balance
        && b > 0.0
    {
        return b;
    }
    // topped_up_balance + granted_balance from UserSummaryData
    let topup = summary.topped_up_balance.unwrap_or(0.0);
    let granted = summary.granted_balance.unwrap_or(0.0);
    if topup + granted > 0.0 {
        return topup + granted;
    }
    // Fallback: sum wallets matching the currency
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
            total += to_f64(&wallet.balance);
        }
    }
    total
}

// ── Monthly cost computation ────────────────────────────────

fn compute_monthly_cost(
    summary: &UserSummaryData,
    currency: &str,
    cost_response: &CostResponse,
) -> f64 {
    if let Some(cost) = summary.month_cost {
        return cost;
    }
    if let Some(costs) = &summary.monthly_costs {
        let matched = costs
            .iter()
            .find(|c| c.currency == currency)
            .or_else(|| costs.first());
        if let Some(c) = matched {
            return to_f64(&c.amount);
        }
    }
    // Fallback: sum from cost bucket total
    match cost_response {
        CostResponse::Bucketed(buckets) => {
            let bucket = pick_cost_bucket(Some(buckets), currency);
            if let Some(b) = bucket {
                return sum_model_entries(&b.total);
            }
        }
        CostResponse::Flat(flat) => {
            return flat.items.iter().map(|i| i.total).sum();
        }
    }
    0.0
}

// ── Cost normalization ──────────────────────────────────────

fn normalize_cost_items(cost: &CostResponse, currency: &str) -> Vec<CostFlatItem> {
    match cost {
        CostResponse::Flat(flat) => flat.items.clone(),
        CostResponse::Bucketed(buckets) => {
            let bucket = match pick_cost_bucket(Some(buckets), currency) {
                Some(b) => b,
                None => return vec![],
            };
            bucket
                .days
                .iter()
                .map(|day| {
                    let models: Vec<ModelCostPayload> = day
                        .data
                        .iter()
                        .map(|entry| ModelCostPayload {
                            name: entry.model.clone(),
                            cost: sum_usage(&entry.usage),
                        })
                        .collect();
                    let total = models.iter().map(|m| m.cost).sum();
                    CostFlatItem {
                        date: day.date.clone(),
                        models,
                        total,
                    }
                })
                .collect()
        }
    }
}

// ── Amount normalization ────────────────────────────────────

fn normalize_amount_items(amount: &AmountResponse) -> Vec<AmountFlatItem> {
    match amount {
        AmountResponse::Flat(flat) => flat.items.clone(),
        AmountResponse::Bucketed(bucket) => bucket
            .days
            .iter()
            .map(|day| {
                let models: Vec<ModelAmountPayload> = day
                    .data
                    .iter()
                    .map(|entry| ModelAmountPayload {
                        name: entry.model.clone(),
                        input_cache_hit: get_usage_amount(&entry.usage, "PROMPT_CACHE_HIT_TOKEN"),
                        input_cache_miss: get_usage_amount(&entry.usage, "PROMPT_CACHE_MISS_TOKEN"),
                        output: get_usage_amount(&entry.usage, "RESPONSE_TOKEN"),
                        api_requests: get_usage_amount(&entry.usage, "REQUEST"),
                    })
                    .collect();
                AmountFlatItem {
                    date: day.date.clone(),
                    models,
                }
            })
            .collect(),
    }
}

// ── Token computation ───────────────────────────────────────

fn compute_token_summary(amount_item: Option<&AmountFlatItem>) -> TokenSummary {
    let (mut cache_hit, mut cache_miss, mut output) = (0u64, 0u64, 0u64);
    if let Some(item) = amount_item {
        for model in &item.models {
            cache_hit += model.input_cache_hit;
            cache_miss += model.input_cache_miss;
            output += model.output;
        }
    }
    let total = cache_hit + cache_miss + output;
    let total_input = cache_hit + cache_miss;
    let cache_hit_rate = if total_input > 0 {
        cache_hit as f64 / total_input as f64
    } else {
        0.0
    };
    TokenSummary {
        input_cache_hit: cache_hit,
        input_cache_miss: cache_miss,
        output,
        total,
        cache_hit_rate,
    }
}

// ── Utility helpers ─────────────────────────────────────────

pub fn format_cost(n: f64, currency: &str) -> String {
    let symbol = if currency == "CNY" { "¥" } else { "$" };
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

fn to_f64(value: &serde_json::Value) -> f64 {
    match value {
        serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0),
        serde_json::Value::String(s) => s.parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    }
}

fn sum_usage(usage: &[UsageMetric]) -> f64 {
    usage.iter().map(|u| to_f64(&u.amount)).sum()
}

fn sum_model_entries(entries: &[ModelUsageEntry]) -> f64 {
    entries.iter().map(|e| sum_usage(&e.usage)).sum()
}

fn get_usage_amount(usage: &[UsageMetric], metric_type: &str) -> u64 {
    usage
        .iter()
        .find(|u| u.metric_type == metric_type)
        .map(|u| to_f64(&u.amount) as u64)
        .unwrap_or(0)
}

fn pick_cost_bucket<'a>(
    buckets: Option<&'a Vec<CostBucket>>,
    currency: &str,
) -> Option<&'a CostBucket> {
    let buckets = buckets?;
    buckets
        .iter()
        .find(|b| b.currency == currency)
        .or_else(|| buckets.first())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_cost_cny() {
        assert_eq!(format_cost(5.76, "CNY"), "¥5.76");
        assert_eq!(format_cost(0.0, "CNY"), "¥0.00");
        assert_eq!(format_cost(100.0, "CNY"), "¥100.00");
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
    fn test_compute_token_summary_empty() {
        let summary = compute_token_summary(None);
        assert_eq!(summary.total, 0);
        assert_eq!(summary.cache_hit_rate, 0.0);
    }

    #[test]
    fn test_compute_token_summary_mixed() {
        let item = AmountFlatItem {
            date: "2026-06-13".into(),
            models: vec![
                ModelAmountPayload {
                    name: "deepseek-chat".into(),
                    input_cache_hit: 1000,
                    input_cache_miss: 500,
                    output: 300,
                    api_requests: 10,
                },
                ModelAmountPayload {
                    name: "deepseek-reasoner".into(),
                    input_cache_hit: 2000,
                    input_cache_miss: 1000,
                    output: 700,
                    api_requests: 5,
                },
            ],
        };
        let summary = compute_token_summary(Some(&item));
        assert_eq!(summary.input_cache_hit, 3000);
        assert_eq!(summary.input_cache_miss, 1500);
        assert_eq!(summary.output, 1000);
        assert_eq!(summary.total, 5500);
        assert!((summary.cache_hit_rate - 0.6666).abs() < 0.01);
    }

    #[test]
    fn test_compute_token_summary_all_miss() {
        let item = AmountFlatItem {
            date: "2026-06-13".into(),
            models: vec![ModelAmountPayload {
                name: "test".into(),
                input_cache_hit: 0,
                input_cache_miss: 100,
                output: 50,
                api_requests: 1,
            }],
        };
        let summary = compute_token_summary(Some(&item));
        assert_eq!(summary.cache_hit_rate, 0.0);
    }

    #[test]
    fn test_compute_token_summary_all_hit() {
        let item = AmountFlatItem {
            date: "2026-06-13".into(),
            models: vec![ModelAmountPayload {
                name: "test".into(),
                input_cache_hit: 100,
                input_cache_miss: 0,
                output: 50,
                api_requests: 1,
            }],
        };
        let summary = compute_token_summary(Some(&item));
        assert_eq!(summary.cache_hit_rate, 1.0);
    }

    #[test]
    fn test_resolve_currency_from_summary() {
        let summary = UserSummaryData {
            currency: Some("USD".into()),
            ..Default::default()
        };
        assert_eq!(
            resolve_currency(&summary, &CostResponse::Bucketed(vec![])),
            "USD"
        );
    }

    #[test]
    fn test_resolve_currency_from_wallet() {
        let summary = UserSummaryData {
            currency: None,
            normal_wallets: Some(vec![WalletBalance {
                currency: "EUR".into(),
                balance: serde_json::Value::Number(serde_json::Number::from(100)),
                token_estimation: None,
            }]),
            ..Default::default()
        };
        assert_eq!(
            resolve_currency(&summary, &CostResponse::Bucketed(vec![])),
            "EUR"
        );
    }

    #[test]
    fn test_resolve_currency_fallback_cny() {
        let summary = UserSummaryData::default();
        assert_eq!(
            resolve_currency(&summary, &CostResponse::Bucketed(vec![])),
            "CNY"
        );
    }

    #[test]
    fn test_normalize_cost_items_flat() {
        let flat = CostFlat {
            month: 6,
            year: 2026,
            items: vec![CostFlatItem {
                date: "2026-06-13".into(),
                models: vec![ModelCostPayload {
                    name: "deepseek-chat".into(),
                    cost: 5.76,
                }],
                total: 5.76,
            }],
        };
        let result = normalize_cost_items(&CostResponse::Flat(flat), "CNY");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].total, 5.76);
    }
}
