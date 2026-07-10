# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`deepseek-cli` is a Rust CLI tool that monitors DeepSeek API usage and costs from the terminal. It authenticates via WeChat QR or manual session token, fetches usage data from four DeepSeek platform endpoints, and renders terminal tables.

## Build & Test Commands

```bash
cargo build --locked              # build
cargo build --locked --release    # release build (~7 MB binary)
cargo fmt                         # format (pre-commit hook)
cargo clippy --all-targets --all-features --tests -D warnings  # lint
cargo test --all-features          # run all tests
cargo test --all-features -v <test_name>  # run single test
cargo nextest run --all-features   # test runner (pre-push hook)
cargo run -- status --no-interactive --period 7d  # quick smoke test
```

No async runtime; builds on stable Rust ≥1.85.

## Architecture

### Module layout

- **`main.rs`** — thin entrypoint: CLI dispatch, interactive date-range resolution, login/token handling. Non-interactive path calls `ApiClient` → `data::aggregate` → `display::show_usage` or JSON output.
- **`cli.rs`** — `clap` `#[derive(Parser)]` enum. `Status` subcommand carries optional `--period`, `--start/--end`, `--verbose`, `--json`, `--no-interactive`.
- **`api.rs`** — `ApiClient` (zero-state) with three methods: `get_user_summary`, `get_usage_by_key_cost`, `get_usage_by_key_amount`. All GET requests with Bearer token and browser User-Agent. Returns `serde_json::Value`, deserialization happens in `types.rs`.
- **`auth/`** — `AuthManager` orchestrates login flows. `wechat.rs` implements WeChat QR (fetch UUID from `open.weixin.qq.com`, download QR image, decode via `rqrr`, re-render via `qrcode`, long-poll scan status, follow OAuth redirect chain, exchange token at `/auth-api/v0/users/oauth/get_token`). `storage.rs` wraps `confy` for token persistence at `~/.config/deepseek-cli/config.toml` with `0600` permissions.
- **`data.rs`** — pure aggregation: merges summary + amount + cost into unified usage models.
- **`types.rs`** — `serde` structs for API responses (browsed types, usage summary, cost/amount breakdowns).
- **`display.rs`** — `tabled` formatting for terminal output.
- **`constants.rs`** — endpoint URLs, User-Agent string.
- **`error.rs`** — `DeepSeekError` enum + `From<ureq::Error>`; `Result<T>` type alias exported from `lib.rs`.

### API response handling

- **Dual schema** — cost/amount endpoints return either Schema A (flat `items[]`) or Schema B (bucketed `total[]`/`days[]`). `api.rs` tries Schema A first, falls back to Schema B via `parse_cost_response` / `parse_amount_response`. `types.rs` defines both structs plus `CostResponse` / `AmountResponse` enums.
- **`biz_data` wrapper** — some responses wrap payload in `{biz_code, biz_msg, biz_data}`. `api.rs` `try_unwrap_biz` extracts `biz_data` and validates `biz_code == 0`; `unwrap_biz_data` does the same during deserialization.
- **`StringOrF64`** — numeric fields (e.g., `monthly_token_usage`) may arrive as JSON numbers or strings. Custom deserializer normalizes to `Option<String>`.
- **Immutable Agent** — `ureq` v3 `Agent` cannot mutate headers after creation. Token is passed per-request via `.header("Authorization", ...)` in `request_json`.

### Key patterns

- **Blocking HTTP via `ureq`** — no async, no `tokio`. All API calls are synchronous.
- **Token stored in config, not env** — `confy` at `~/.config/deepseek-cli/config.toml`. Always read via `storage::get_token()`, never hardcode or read from env vars.
- **Interactive fallbacks** — `main.rs` `resolve_time_range` cascades: CLI flags → JSON/no-interactive defaults → `inquire` Select/DateSelect prompts. When `--json` or `--no-interactive` is set, no prompts run.
- **Date range limit** — 30 days max; enforced in `resolve_time_range` and `prompt_custom_dates`.
- **Single binary, no runtime deps** — `ureq` compiles to native-tls by default; distribution profile uses `lto = "thin"`.

## Pre-commit Hooks

Hooks run on commit: `cargo fmt`, `cargo clippy -D warnings`. On push: `cargo test`, `cargo build --locked`, `cargo check`.

## Dist

`cargo dist` build defined in `dist-workspace.toml`. Built with `--profile dist` (`lto = "thin"`).
