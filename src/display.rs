use crate::data::{format_cost, format_number, format_tokens};
use crate::types::AggregatedData;
use tabled::Table;
use tabled::settings::Style;

/// Format cache hit rate as percentage string.
fn format_hit_rate(rate: f64) -> String {
    format!("{:.1}%", rate * 100.0)
}

/// Print the full usage dashboard to stdout.
pub fn show_usage(data: &AggregatedData, verbose: bool) {
    let beijing = data.last_updated + chrono::Duration::hours(8);
    let time_str = beijing.format("%Y-%m-%d %H:%M:%S").to_string();

    println!();
    println!("  DeepSeek Usage · {} CST", beijing.format("%Y-%m-%d"));
    println!();

    // ── Summary table ────────────────────────────────────
    let summary_rows = vec![
        SummaryRow {
            item: "Balance",
            amount: format!(
                "{} {}",
                format_cost(data.balance, &data.currency),
                data.currency
            ),
        },
        SummaryRow {
            item: "Period Cost",
            amount: format_cost(data.period_cost, &data.currency),
        },
        SummaryRow {
            item: "API Requests",
            amount: format_number(data.period_api_requests),
        },
        SummaryRow {
            item: "Tokens",
            amount: format_tokens(data.period_tokens),
        },
        SummaryRow {
            item: "Cache Hit Rate",
            amount: format_hit_rate(data.cache_hit_rate),
        },
    ];
    println!("{}", Table::new(summary_rows).with(Style::rounded()));

    // ── Model breakdown ──────────────────────────────────
    if !data.models.is_empty() {
        println!();
        println!("  Usage by Model");
        println!();
        let model_rows: Vec<ModelRow> = data
            .models
            .iter()
            .map(|m| ModelRow {
                model: m.name.clone(),
                cost: format_cost(m.cost, &data.currency),
                api_requests: format_number(m.api_requests),
                tokens: format_tokens(m.tokens),
                hit_rate: if m.cache_hit + m.cache_miss > 0 {
                    format_hit_rate(m.cache_hit as f64 / (m.cache_hit + m.cache_miss) as f64)
                } else {
                    "-".into()
                },
            })
            .collect();
        println!("{}", Table::new(model_rows).with(Style::rounded()));
    }

    // ── Daily breakdown (verbose) ────────────────────────
    if verbose && !data.daily_items.is_empty() {
        println!();
        println!("  Daily Breakdown");
        println!();
        let daily_rows: Vec<DailyRow> = data
            .daily_items
            .iter()
            .map(|d| DailyRow {
                date: d.date.clone(),
                cost: format_cost(d.cost, &data.currency),
                api_requests: format_number(d.api_requests),
                tokens: format_tokens(d.tokens),
                output_tokens: format_tokens(d.output_tokens),
                cache_hit: format_tokens(d.cache_hit),
                cache_miss: format_tokens(d.cache_miss),
            })
            .collect();
        println!("{}", Table::new(daily_rows).with(Style::rounded()));
    }

    println!();
    println!("  Updated: {} CST", time_str);
    println!();
}

// ── Table row types ─────────────────────────────────────────

#[derive(tabled::Tabled)]
#[tabled(rename_all = "PascalCase")]
struct SummaryRow {
    item: &'static str,
    amount: String,
}

#[derive(tabled::Tabled)]
#[tabled(rename_all = "PascalCase")]
struct ModelRow {
    model: String,
    cost: String,
    api_requests: String,
    tokens: String,
    hit_rate: String,
}

#[derive(tabled::Tabled)]
#[tabled(rename_all = "PascalCase")]
struct DailyRow {
    date: String,
    cost: String,
    api_requests: String,
    tokens: String,
    output_tokens: String,
    cache_hit: String,
    cache_miss: String,
}

// ── QR code display ─────────────────────────────────────────

/// Render a QR code in the terminal using Unicode half-block characters.
pub fn show_qr_code(qr_url: &str) {
    use qrcode::QrCode;
    use qrcode::render::unicode::Dense1x2;

    println!();
    match QrCode::new(qr_url) {
        Ok(code) => {
            let qr_str = code
                .render::<Dense1x2>()
                .dark_color(Dense1x2::Dark)
                .light_color(Dense1x2::Light)
                .build();
            println!("{}", qr_str);
        }
        Err(e) => {
            eprintln!("Failed to generate QR code: {}", e);
            return;
        }
    }
    println!();
    println!("  Scan the QR code above with WeChat to log in.");
    println!();
}

// ── Login status display ────────────────────────────────────

pub fn show_login_status(status: &str) {
    match status {
        "fetching" => eprintln!("Fetching WeChat QR code..."),
        "waiting" => eprintln!("Waiting for scan. Scan the QR code with WeChat..."),
        "scanned" => println!("\n  ✓ Scanned! Waiting for phone confirmation..."),
        "confirmed" => println!("\n  ✓ Login confirmed! Exchanging for session token..."),
        "success" => println!("\n  ✓ Login successful!"),
        "expired" => eprintln!("\n  ✗ QR code expired. Please try again."),
        "error" => eprintln!("\n  ✗ Login failed."),
        _ => {}
    }
}
