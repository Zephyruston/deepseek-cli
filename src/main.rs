use std::io::{self, Write};

use chrono::{Datelike, Utc};
use clap::{CommandFactory, Parser};
use clap_complete::generate;

use deepseek_cli::Result;
use deepseek_cli::api::ApiClient;
use deepseek_cli::auth::AuthManager;
use deepseek_cli::auth::storage;
use deepseek_cli::cli::{Cli, Commands};
use deepseek_cli::data;
use deepseek_cli::display;
use deepseek_cli::error::DeepSeekError;

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Status { verbose } => handle_status(verbose),
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

fn handle_status(verbose: bool) -> Result<()> {
    let token = storage::get_token()?;
    let api = ApiClient::new();
    do_fetch_and_display(&api, &token, verbose)
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

fn do_fetch_and_display(api: &ApiClient, token: &str, verbose: bool) -> Result<()> {
    let now = Utc::now();
    let month = now.month() as i32;
    let year = now.year();

    let summary = api.get_user_summary(token)?;
    let cost = api.get_usage_cost(token, month, year)?;
    let amount = api.get_usage_amount(token, month, year)?;

    let aggregated = data::aggregate(summary, &cost, &amount, now);
    display::show_usage(&aggregated, verbose);
    Ok(())
}

fn handle_completions(shell: clap_complete::Shell) -> Result<()> {
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    generate(shell, &mut cmd, name, &mut std::io::stdout());
    Ok(())
}
