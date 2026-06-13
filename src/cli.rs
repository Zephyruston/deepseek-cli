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
    /// Show usage summary (balance, monthly cost, today cost, token usage)
    Status {
        /// Show per-model cost breakdown
        #[arg(short, long)]
        verbose: bool,
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
