use crate::data::{format_cost, format_tokens};
use crate::types::AggregatedData;
use tabled::Table;
use tabled::settings::Style;

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
            item: "Monthly Cost",
            amount: format_cost(data.monthly_cost, &data.currency),
        },
        SummaryRow {
            item: "Today Cost",
            amount: format_cost(data.today_cost, &data.currency),
        },
    ];
    println!("{}", Table::new(summary_rows).with(Style::rounded()));

    // ── Model breakdown ──────────────────────────────────
    if verbose && !data.today_cost_by_model.is_empty() {
        println!();
        println!("  Today's Cost by Model");
        println!();
        let model_rows: Vec<ModelRow> = data
            .today_cost_by_model
            .iter()
            .map(|m| ModelRow {
                model: m.name.clone(),
                cost: format_cost(m.cost, &data.currency),
            })
            .collect();
        println!("{}", Table::new(model_rows).with(Style::rounded()));
    }

    // ── Token usage ──────────────────────────────────────
    println!();
    println!(
        "  Today's Token Usage ({})",
        format_tokens(data.today_tokens.total)
    );
    println!();
    let token_rows = vec![
        TokenRow {
            metric_type: "Input (Cache Hit)",
            count: format_tokens(data.today_tokens.input_cache_hit),
        },
        TokenRow {
            metric_type: "Input (Cache Miss)",
            count: format_tokens(data.today_tokens.input_cache_miss),
        },
        TokenRow {
            metric_type: "Output",
            count: format_tokens(data.today_tokens.output),
        },
        TokenRow {
            metric_type: "API Requests",
            count: format_tokens(data.today_api_requests),
        },
    ];
    println!("{}", Table::new(token_rows).with(Style::rounded()));

    if data.today_tokens.total > 0 {
        let rate = (data.today_tokens.cache_hit_rate * 100.0).round();
        println!();
        println!("  Cache Hit Rate: {}%", rate);
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
}

#[derive(tabled::Tabled)]
#[tabled(rename_all = "PascalCase")]
struct TokenRow {
    #[tabled(rename = "Type")]
    metric_type: &'static str,
    count: String,
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
