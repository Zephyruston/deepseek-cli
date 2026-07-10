# DeepSeek CLI

Monitor [DeepSeek API](https://platform.deepseek.com) usage and costs directly from your terminal. No browser. No runtime dependencies.

<p align="center">
  <img src="https://img.shields.io/badge/rust-1.85+-orange?logo=rust" alt="Rust">
  <img src="https://img.shields.io/badge/license-MIT-blue" alt="License">
</p>

[中文文档](README_zh.md)

> This is the Rust CLI edition. For the VS Code extension, see [deepseek-monitor-plugin](https://gitee.com/sundawei/deepseek-monitor-plugin).

---

## Overview

`deepseek-cli` pulls your DeepSeek platform data — balance, monthly spend, daily cost, token volume, and cache hit rate — and displays it as clean terminal tables. Authenticate once with WeChat QR and you're done.

### Features

- **One command** — `deepseek status` shows everything: balance, period cost, API requests, tokens, per-model breakdown
- **Time range control** — `--period today` for today (default), `--period 7d` for last 7 days, `--period 30d` for last 30 days, etc.
  - Or just run `deepseek status` and pick interactively (including a calendar date picker for custom ranges)
- **Machine-readable** — `--json` outputs raw data for scripting / piping
- **Daily breakdown** — `-v` adds a table of daily cost, requests, and tokens
- **WeChat QR login** — scan a QR straight from your terminal, no copy-paste of tokens
- **Single binary** — ~7 MB, statically linked, zero runtime dependencies

### Quick demo

```
$ deepseek login
# QR code renders in terminal → scan with WeChat → logged in

$ deepseek status --period 30d
  DeepSeek Usage · 2026-06-13 CST

╭──────────────┬────────────╮
│ Item         │ Amount     │
├──────────────┼────────────┤
│ Balance      │ ¥78.14 CNY │
│ Period Cost  │ ¥26.04     │
│ API Requests │ 2,782      │
│ Tokens       │ 275.65M    │
╰──────────────┴────────────╯

  Usage by Model

╭───────────────────────────────────┬───────┬─────────────┬──────────╮
│ Model                             │ Cost  │ ApiRequests │ Tokens   │
├───────────────────────────────────┼───────┼─────────────┼──────────┤
│ deepseek-v4-pro                   │ ¥18.04│ 1,158       │ 151.66M  │
│ deepseek-v4-flash                 │ ¥8.00 │ 1,624       │ 123.99M  │
│ deepseek-chat & deepseek-reasoner │ ¥0.00 │ 0           │ 0        │
╰───────────────────────────────────┴───────┴─────────────┴──────────╯

$ deepseek status --period 30d -v
  ...adds daily breakdown table with date, cost, requests, tokens per day
```

## Install

```bash
# From source (Rust ≥1.85)
git clone https://github.com/Zephyruston/deepseek-cli.git
cd deepseek-cli
cargo install --path . --locked
```

### Shell completions

```bash
# bash
echo 'source <(deepseek completions bash)' >> ~/.bashrc

# zsh
echo 'source <(deepseek completions zsh)' >> ~/.zshrc

# fish
deepseek completions fish > ~/.config/fish/completions/deepseek.fish
```

## Commands

| Command                                               | Description                                 |
| ----------------------------------------------------- | ------------------------------------------- |
| `deepseek status`                                     | Usage dashboard (defaults to today)         |
| `deepseek status --period today`                      | Today                                       |
| `deepseek status --period 7d`                         | Last 7 days                                 |
| `deepseek status --period 30d`                        | Last 30 days                                |
| `deepseek status --period this-month`                 | Current month                               |
| `deepseek status --period last-month`                 | Previous month                              |
| `deepseek status --start YYYY-MM-DD --end YYYY-MM-DD` | Custom date range (max 30 days)             |
| `deepseek status -v`                                  | Add daily breakdown table                   |
| `deepseek status --json`                              | Output as JSON (for scripting)              |
| `deepseek status --no-interactive`                    | Disable interactive prompts (default today) |
| `deepseek login`                                      | WeChat QR authentication                    |
| `deepseek token`                                      | Paste session token manually                |
| `deepseek logout`                                     | Clear stored credentials                    |
| `deepseek completions <SHELL>`                        | Generate shell completion script            |

## Authentication

### WeChat QR (recommended)

```bash
deepseek login
```

A QR code renders in your terminal. Scan with WeChat, confirm on your phone. The session token is saved automatically.

### Manual token

```bash
deepseek token
# Paste your session token (from platform.deepseek.com)
```

The token is validated against the API before saving.

## How it works

Four API endpoints (all `GET`, Bearer auth, browser User-Agent):

| Endpoint                                             | Provides                                                |
| ---------------------------------------------------- | ------------------------------------------------------- |
| `/api/v0/users/get_user_summary`                     | Balance, currency, monthly cost, wallet info            |
| `/api/v0/usage/by_api_key/cost?start=S&end=E&tz=0`   | Daily cost per model for a time range (Unix timestamps) |
| `/api/v0/usage/by_api_key/amount?start=S&end=E&tz=0` | Daily token volume, cache hit/miss, API requests        |
| `/api/v0/users/get_api_keys`                         | List of API keys                                        |

The login flow:

1. Fetch the WeChat QR connect page from `open.weixin.qq.com` → extract UUID
2. Download the real WeChat QR image → decode content with `rqrr` → re-render in terminal with `qrcode` (Unicode half-blocks)
3. Long-poll `long.open.weixin.qq.com` for scan status (adaptive 2s / 100ms intervals, 5 min timeout)
4. Follow OAuth redirect chain → extract `nonce` + `provider`
5. Exchange for session token via `POST /auth-api/v0/users/oauth/get_token`

Token stored at `~/.config/deepseek-cli/config.toml` (permissions `0600`).

## Dependencies

| Crate                  | Role                                  |
| ---------------------- | ------------------------------------- |
| `ureq`                 | HTTP client (blocking, cookies, JSON) |
| `clap`                 | CLI argument parsing                  |
| `serde` / `serde_json` | JSON deserialization                  |
| `chrono`               | UTC dates, Beijing time formatting    |
| `tabled`               | Terminal table rendering              |
| `qrcode`               | QR code generation (Unicode renderer) |
| `rqrr`                 | QR code decoding (from WeChat image)  |
| `image`                | PNG/JPEG image loading                |
| `regex`                | HTML/JS response text parsing         |
| `confy`                | Cross-platform config storage         |
| `clap_complete`        | Shell completion generation           |
| `thiserror`            | Error type derivation                 |

No async runtime. No OpenSSL. Linux binary ~7 MB.

## Requirements

- Rust ≥1.85 (build only)
- Terminal with Unicode support
- DeepSeek Platform account

## License

[MIT](./LICENSE)
