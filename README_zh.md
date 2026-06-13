# DeepSeek CLI

在终端直接查看 [DeepSeek API](https://platform.deepseek.com) 用量和费用，无需浏览器，零运行时依赖。

<p align="center">
  <img src="https://img.shields.io/badge/rust-1.85+-orange?logo=rust" alt="Rust">
  <img src="https://img.shields.io/badge/license-MIT-blue" alt="License">
</p>

[English](README.md)

> 这是 Rust CLI 版本。VS Code 插件版见 [deepseek-monitor-plugin](https://gitee.com/sundawei/deepseek-monitor-plugin)。

---

## 概述

`deepseek-cli` 从 DeepSeek 平台拉取你的 API 用量数据——余额、月消费、日消费、Token 用量、缓存命中率——以终端表格展示。一次微信扫码登录即可，无需手动复制 token。

### 功能

- **一行命令看全部** — `deepseek status` 显示余额、月消费、日消费、Token 用量、缓存命中率
- **按模型拆分** — `-v` 追加各模型费用明细（v4-pro、v4-flash、chat 等）
- **微信扫码登录** — 终端直接渲染二维码，扫码即登录
- **单文件二进制** — 约 7 MB，静态链接，无需任何运行时

### 效果演示

```
$ deepseek login
# 终端渲染二维码 → 微信扫描 → 登录成功

$ deepseek status -v
  DeepSeek Usage · 2026-06-13 CST

╭──────────────┬───────────╮
│ Item         │ Amount    │
├──────────────┼───────────┤
│ Balance      │ ¥8.77 CNY │
│ Monthly Cost │ ¥1.39     │
│ Today Cost   │ ¥0.00     │
╰──────────────┴───────────╯

  Today's Cost by Model

╭───────────────────────────────────┬───────╮
│ Model                             │ Cost  │
├───────────────────────────────────┼───────┤
│ deepseek-v4-pro                   │ ¥0.00 │
│ deepseek-v4-flash                 │ ¥0.00 │
│ deepseek-chat & deepseek-reasoner │ ¥0.00 │
╰───────────────────────────────────┴───────╯

  Today's Token Usage (44.85M)

╭────────────────────┬──────────╮
│ Type               │ Count    │
├────────────────────┼──────────┤
│ Input (Cache Hit)  │ 43.86M   │
│ Input (Cache Miss) │ 718.7K   │
│ Output             │ 270.8K   │
│ API Requests       │ 4,528    │
╰────────────────────┴──────────╯

  Cache Hit Rate: 98.4%
  Updated: 2026-06-13 18:30:00 CST
```

## 安装

```bash
# 源码编译（需要 Rust ≥1.85）
git clone https://github.com/Zephyruston/deepseek-cli.git
cd deepseek-cli
cargo install --path .
```

### Shell 补全

```bash
# bash
echo 'source <(deepseek completions bash)' >> ~/.bashrc

# zsh
echo 'source <(deepseek completions zsh)' >> ~/.zshrc

# fish
deepseek completions fish > ~/.config/fish/completions/deepseek.fish
```

## 命令

| 命令                           | 说明                   |
| ------------------------------ | ---------------------- |
| `deepseek status`              | 用量看板               |
| `deepseek status -v`           | 含各模型费用拆分       |
| `deepseek login`               | 微信扫码登录           |
| `deepseek token`               | 手动粘贴 session token |
| `deepseek logout`              | 清除登录凭证           |
| `deepseek completions <SHELL>` | 生成 shell 补全脚本    |

## 认证

### 微信扫码（推荐）

```bash
deepseek login
```

终端渲染二维码 → 微信扫描 → 手机确认 → 自动保存 token。

### 手动粘贴 Token

```bash
deepseek token
# 粘贴从 platform.deepseek.com 获取的 session token
```

Token 存储前会调用 API 验证有效性。

## 实现原理

数据来自三个 API 端点（均为 GET，Bearer 认证）：

| 端点                                  | 提供                                       |
| ------------------------------------- | ------------------------------------------ |
| `/api/v0/users/get_user_summary`      | 余额、货币类型、月度消费、钱包信息         |
| `/api/v0/usage/cost?month=M&year=Y`   | 按日各模型费用                             |
| `/api/v0/usage/amount?month=M&year=Y` | 按日 Token 量、缓存命中/未命中、API 请求数 |

登录流程：

1. 从 `open.weixin.qq.com` 获取微信扫码页面 → 解析会话 UUID
2. 下载微信真实 QR 图片 → 用 `rqrr` 解码内容 → 用 `qrcode` crate（Unicode 半块字符）在终端重新渲染
3. 长轮询 `long.open.weixin.qq.com` 监听扫码状态（自适应 2s / 100ms 间隔，最长 5 分钟）
4. 跟随 OAuth 重定向链 → 提取 `nonce` + `provider`
5. 用 `POST /auth-api/v0/users/oauth/get_token` 换取 session token

Token 保存于 `~/.config/deepseek-cli/config.toml`（权限 `0600`）。

## 依赖

| Crate                  | 用途                                     |
| ---------------------- | ---------------------------------------- |
| `ureq`                 | HTTP 客户端（阻塞式，cookie 管理，JSON） |
| `clap`                 | 命令行参数解析                           |
| `serde` / `serde_json` | JSON 反序列化                            |
| `chrono`               | UTC 日期、北京时间格式化                 |
| `tabled`               | 终端表格渲染                             |
| `qrcode`               | QR 码生成（Unicode 渲染器）              |
| `rqrr`                 | QR 码解码（从微信图片提取内容）          |
| `image`                | PNG/JPEG 图片加载                        |
| `regex`                | HTML/JS 响应文本解析                     |
| `confy`                | 跨平台配置存储                           |
| `clap_complete`        | Shell 补全脚本生成                       |
| `thiserror`            | 错误类型派生                             |

不含异步框架，不含 OpenSSL。Linux 二进制约 7 MB。

## 环境要求

- Rust ≥1.85（仅编译时需要）
- 支持 Unicode 的终端
- DeepSeek 平台账号

## 开源协议

[MIT](./LICENSE)
