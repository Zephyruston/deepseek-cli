use std::io::{self, Write};

use chrono::Utc;
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use inquire::{DateSelect, InquireError, Select};

use deepseek_cli::Result;
use deepseek_cli::api::ApiClient;
use deepseek_cli::auth::AuthManager;
use deepseek_cli::auth::storage;
use deepseek_cli::cli::{Cli, Commands};
use deepseek_cli::data::{self, compute_time_range, compute_time_range_from_dates};
use deepseek_cli::display;
use deepseek_cli::error::DeepSeekError;

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Status {
            period,
            start,
            end,
            verbose,
            json,
            no_interactive,
        } => handle_status(period, start, end, verbose, json, no_interactive),
        Commands::Login => handle_login(),
        Commands::Logout => AuthManager::logout(),
        Commands::SetToken { value } => handle_set_token(value),
        Commands::Completions { shell } => handle_completions(shell),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn handle_status(
    period: Option<String>,
    start_cli: Option<String>,
    end_cli: Option<String>,
    verbose: bool,
    json: bool,
    no_interactive: bool,
) -> Result<()> {
    let token = storage::get_token()?;
    let api = ApiClient::new();

    // --start/--end takes precedence for custom range
    let (start_ts, end_ts) = resolve_time_range(period, start_cli, end_cli, no_interactive, json)?;
    do_fetch_and_display(&api, &token, start_ts, end_ts, verbose, json)
}

fn resolve_time_range(
    period: Option<String>,
    start_cli: Option<String>,
    end_cli: Option<String>,
    no_interactive: bool,
    json: bool,
) -> Result<(i64, i64)> {
    // CLI --start/--end takes precedence
    if let (Some(s), Some(e)) = (&start_cli, &end_cli) {
        let range = compute_time_range_from_dates(s, e)?;
        let days = (range.1 - range.0) / 86400;
        if days > 30 {
            return Err(DeepSeekError::Parse(
                "date range cannot exceed 30 days".into(),
            ));
        }
        return Ok(range);
    }
    // Partial --start/--end without the other is an error
    if start_cli.is_some() || end_cli.is_some() {
        return Err(DeepSeekError::Parse(
            "--start and --end must be used together".into(),
        ));
    }
    // CLI --period takes precedence
    if let Some(p) = period {
        return Ok(compute_time_range(&p));
    }
    // JSON mode or no-interactive: use default
    if json || no_interactive {
        return Ok(compute_time_range("7d"));
    }
    // Interactive prompt
    let options = vec![
        "7d — Last 7 days (default)",
        "30d — Last 30 days",
        "this-month — Current month",
        "last-month — Previous month",
        "custom — Pick a date range",
    ];
    let choice = Select::new("Select time period:", options)
        .with_starting_cursor(0)
        .with_vim_mode(false);
    match choice.prompt() {
        Ok(selected) => {
            let key = selected.split(" — ").next().unwrap_or("7d").trim();
            match key {
                "custom" => prompt_custom_dates(),
                _ => Ok(compute_time_range(key)),
            }
        }
        Err(InquireError::OperationCanceled) => {
            eprintln!("Canceled.");
            std::process::exit(0);
        }
        Err(InquireError::OperationInterrupted) => {
            eprintln!("\nCanceled.");
            std::process::exit(0);
        }
        Err(e) => Err(DeepSeekError::Parse(format!(
            "interactive prompt failed: {}",
            e
        ))),
    }
}

fn prompt_custom_dates() -> Result<(i64, i64)> {
    let today = Utc::now().date_naive();

    let start_date = DateSelect::new("Start date:")
        .with_starting_date(today - chrono::Duration::days(29))
        .with_min_date(today - chrono::Duration::days(365))
        .with_max_date(today)
        .prompt()
        .map_err(|e| match e {
            InquireError::OperationCanceled | InquireError::OperationInterrupted => {
                eprintln!("Canceled.");
                std::process::exit(0);
            }
            _ => DeepSeekError::Parse(format!("date prompt failed: {}", e)),
        })?;

    // API supports max 30 days range
    let max_end = start_date + chrono::Duration::days(30);
    let effective_max = if max_end > today { today } else { max_end };

    let end_date = DateSelect::new("End date:")
        .with_starting_date(start_date)
        .with_min_date(start_date)
        .with_max_date(effective_max)
        .prompt()
        .map_err(|e| match e {
            InquireError::OperationCanceled | InquireError::OperationInterrupted => {
                eprintln!("Canceled.");
                std::process::exit(0);
            }
            _ => DeepSeekError::Parse(format!("date prompt failed: {}", e)),
        })?;

    let days = (end_date - start_date).num_days();
    if days > 30 {
        return Err(DeepSeekError::Parse(
            "date range cannot exceed 30 days".into(),
        ));
    }

    let start_ts = start_date
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp();
    let end_exclusive = end_date + chrono::Duration::days(1);
    let end_ts = end_exclusive
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp();

    Ok((start_ts, end_ts))
}

fn handle_login() -> Result<()> {
    let mut auth = AuthManager::new();
    auth.login_interactive()
}

fn handle_set_token(value: Option<String>) -> Result<()> {
    let token = match value {
        Some(t) => t,
        None => {
            eprint!("Paste your DeepSeek session token: ");
            io::stderr().flush().ok();
            let mut input = String::new();
            io::stdin()
                .read_line(&mut input)
                .map_err(|e| DeepSeekError::TokenStorage(format!("failed to read input: {}", e)))?;
            input.trim().to_string()
        }
    };
    AuthManager::set_token(&token)
}

fn do_fetch_and_display(
    api: &ApiClient,
    token: &str,
    start_ts: i64,
    end_ts: i64,
    verbose: bool,
    json: bool,
) -> Result<()> {
    let now = Utc::now();

    let summary = api.get_user_summary(token)?;
    let cost = api.get_usage_by_key_cost(token, start_ts, end_ts, 0)?;
    let amount = api.get_usage_by_key_amount(token, start_ts, end_ts, 0)?;

    let aggregated = data::aggregate(summary, &amount, &cost, now);
    if json {
        println!("{}", serde_json::to_string_pretty(&aggregated).unwrap());
    } else {
        display::show_usage(&aggregated, verbose);
    }
    Ok(())
}

fn handle_completions(shell: clap_complete::Shell) -> Result<()> {
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    generate(shell, &mut cmd, name, &mut std::io::stdout());
    Ok(())
}
