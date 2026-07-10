use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "deepseek",
    about = "Monitor DeepSeek API usage and costs from the terminal",
    version,
    long_about = "Fetch DeepSeek API usage data (balance, costs, token usage) and display in terminal tables. Supports WeChat QR login for authentication."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Show usage summary (balance, period cost, API requests, tokens, model breakdown)
    Status {
        /// Time period to display
        /// 7d = last 7 days (default), 30d = last 30 days,
        /// this-month, last-month, custom
        #[arg(short, long)]
        period: Option<String>,
        /// Start date for custom range (YYYY-MM-DD). Requires --end.
        #[arg(long)]
        start: Option<String>,
        /// End date for custom range (YYYY-MM-DD). Requires --start.
        #[arg(long)]
        end: Option<String>,
        /// Show per-model breakdown and daily details
        #[arg(short, long)]
        verbose: bool,
        /// Output as JSON instead of table
        #[arg(long)]
        json: bool,
        /// Disable interactive prompts (use defaults)
        #[arg(long)]
        no_interactive: bool,
    },

    /// Log in with WeChat QR code
    Login,

    /// Log out and clear stored token
    Logout,

    /// Manually set the session token
    #[command(name = "token")]
    SetToken {
        /// Session token string. If not provided, read from stdin.
        #[arg(short, long)]
        value: Option<String>,
    },

    /// Generate shell completion script
    #[command(name = "completions")]
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}
