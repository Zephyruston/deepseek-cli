use std::time::{Duration, Instant};

use regex::Regex;

use crate::Result;
use crate::constants::*;
use crate::error::DeepSeekError;

/// Session returned from fetch_qr_code.
pub struct QrCodeSession {
    pub uuid: String,
    /// The content decoded from WeChat's QR image (used to re-render locally).
    pub qr_content: String,
}

// ── Step 1: Fetch QR code ───────────────────────────────────

/// Fetch the WeChat QR connect page, extract UUID, download the QR PNG,
/// decode its content so we can re-render it cleanly in terminal.
pub fn fetch_qr_code(agent: &ureq::Agent) -> Result<QrCodeSession> {
    let ts = chrono::Utc::now().timestamp_millis();

    let redirect_uri = format!(
        "{}/auth-api/v0/users/oauth/wechat/callback",
        DEEPSEEK_BASE_URL
    );

    // Step 1: GET the QR connect page to get UUID and establish cookies
    let html = agent
        .get(&format!("{}/connect/qrconnect", WECHAT_OPEN_BASE))
        .query("appid", WECHAT_APP_ID)
        .query("scope", "snsapi_login")
        .query("redirect_uri", &redirect_uri)
        .query("state", "")
        .query("login_type", "jssdk")
        .query("self_redirect", "false")
        .query("stylelite", "1")
        .query("fast_login", "0")
        .query("ts", ts.to_string())
        .query("styletype", "")
        .query("sizetype", "")
        .query("bgcolor", "")
        .query("rst", "")
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        )
        .header(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
        .call()
        .map_err(DeepSeekError::from)?
        .into_body().read_to_string()
        .map_err(|e| DeepSeekError::QrLogin(format!("failed to read QR page: {}", e)))?;

    let uuid = parse_uuid(&html).ok_or_else(|| {
        let snippet: String = html
            .chars()
            .filter(|c| !c.is_whitespace())
            .take(320)
            .collect();
        DeepSeekError::QrLogin(format!(
            "failed to extract UUID from QR page. snippet: {}",
            snippet
        ))
    })?;

    // Step 2: Download the actual QR image from WeChat
    let qr_bytes = agent
        .get(&format!("{}/connect/qrcode/{}", WECHAT_OPEN_BASE, uuid))
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        )
        .header("Accept", "image/png,image/jpeg,image/*")
        .call()
        .map_err(|e| DeepSeekError::QrLogin(format!("failed to download QR image: {}", e)))?
        .into_body()
        .read_to_vec()
        .map_err(|e| DeepSeekError::QrLogin(format!("failed to read QR image bytes: {}", e)))?;

    // Step 3: Decode the QR content from the image
    let qr_content = decode_qr(&qr_bytes)
        .ok_or_else(|| DeepSeekError::QrLogin("failed to decode WeChat QR code image".into()))?;

    Ok(QrCodeSession { uuid, qr_content })
}

/// Decode QR code content from image bytes (PNG or JPEG).
fn decode_qr(bytes: &[u8]) -> Option<String> {
    let img = image::load_from_memory(bytes).ok()?;
    let gray = img.into_luma8();
    let mut qr_img = rqrr::PreparedImage::prepare(gray);
    let grids = qr_img.detect_grids();
    grids.first()?.decode().ok().map(|(_, content)| content)
}

// ── Step 2: Poll for scan ───────────────────────────────────

/// Long-poll for WeChat scan status.
/// Returns the wx_code on confirmed, or an error on expiry/failure.
pub fn poll_for_login(agent: &ureq::Agent, uuid: &str) -> Result<String> {
    let start = Instant::now();
    let max_duration = Duration::from_secs(POLL_MAX_DURATION_SECS);
    let mut last_code: Option<i32> = None;
    let mut attempt: u32 = 0;

    loop {
        if start.elapsed() > max_duration {
            return Err(DeepSeekError::PollTimeout(POLL_MAX_DURATION_SECS));
        }

        attempt += 1;
        let url = format!("{}/connect/l/qrconnect", WECHAT_LONG_POLL_BASE);

        let mut req = agent
            .get(&url)
            .query("uuid", uuid)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            )
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            );

        if let Some(last) = last_code {
            req = req.query("last", last.to_string());
        }

        let body = match req.call() {
            Ok(resp) => resp.into_body().read_to_string().unwrap_or_default(),
            Err(e) => {
                // Network error — retry a few times
                if attempt <= 5 {
                    std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
                    continue;
                }
                return Err(DeepSeekError::from(e));
            }
        };

        let poll_code = parse_poll_code(&body);
        let wx_code = parse_wx_code(&body);

        // If we have wx_code, it's confirmed
        if let Some(wx) = wx_code {
            return Ok(wx);
        }

        match poll_code {
            408 => {
                // Still waiting
            }
            404 | 201 => {
                // Scanned — poll faster next iteration
            }
            200 => {
                // Confirmed — try to build redirect from body
                if let Some(wx) = parse_wx_code(&body) {
                    return Ok(wx);
                }
                if let Some(redirect) = parse_redirect_uri(&body) {
                    return Ok(redirect);
                }
                return Err(DeepSeekError::QrLogin(
                    "scan confirmed but no auth code in response".into(),
                ));
            }
            402 | 403 => {
                return Err(DeepSeekError::QrLogin(
                    "QR code expired or cancelled".into(),
                ));
            }
            405 => {
                return Err(DeepSeekError::QrLogin(
                    "WeChat confirmed but no wx_code returned (405)".into(),
                ));
            }
            500 => {
                // Transient server error — treat as waiting
            }
            _ => {
                if attempt > 3 {
                    return Err(DeepSeekError::QrLogin(format!(
                        "unexpected poll code: {}",
                        poll_code
                    )));
                }
            }
        }

        last_code = Some(poll_code);

        // Adaptive interval
        let is_scanned = matches!(last_code, Some(404 | 201));
        let interval = if is_scanned {
            POLL_FAST_INTERVAL_MS
        } else {
            POLL_INTERVAL_MS
        };
        std::thread::sleep(Duration::from_millis(interval));
    }
}

// ── Step 3: OAuth callback → token exchange ─────────────────

/// Handle the OAuth callback chain: follow redirects, extract nonce+provider,
/// exchange for session token.
/// Uses a separate agent with redirects disabled so we can inspect intermediate URLs.
pub fn handle_oauth_callback(agent: &ureq::Agent, wx_code: &str) -> Result<String> {
    // Create a no-redirect agent for manual redirect following
    let no_redirect = ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(15)))
            .http_status_as_error(false)
            .max_redirects(0)
            .build(),
    );

    let redirect_uri = format!(
        "{}/auth-api/v0/users/oauth/wechat/callback?code={}&state=",
        DEEPSEEK_BASE_URL, wx_code
    );

    let mut current_url = redirect_uri;
    let mut exchange_info: Option<OAuthExchangeInfo> = None;
    let mut last_body: Option<String> = None;

    // Follow up to 6 redirects
    for _ in 0..6 {
        let response = no_redirect
            .get(&current_url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )
            .header("Accept", "text/html,application/json,*/*")
            .call();

        match response {
            Ok(resp) => {
                let status = resp.status();
                let location = resp
                    .headers()
                    .get("location")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());
                let body = resp.into_body().read_to_string().unwrap_or_default();

                exchange_info = parse_oauth_info(&current_url, location.as_deref(), &body);
                if exchange_info.is_some() {
                    break;
                }

                last_body = Some(body.clone());

                if status.is_redirection()
                    && let Some(loc) = location
                {
                    current_url = loc;
                    continue;
                }
                break;
            }
            Err(e) => {
                return Err(DeepSeekError::QrLogin(format!(
                    "OAuth callback request failed: {}",
                    e
                )));
            }
        }
    }

    // Exchange nonce+provider for token
    if let Some(info) = exchange_info {
        return exchange_oauth_token(agent, &info.nonce, &info.provider);
    }

    // Fallback: try embedded token in last response body
    if let Some(ref body) = last_body
        && let Some(token) = parse_embedded_token(body)
    {
        return Ok(token);
    }

    Err(DeepSeekError::OAuthFailed(
        "could not find nonce/provider in OAuth callback chain".into(),
    ))
}

// ── Token exchange POST ─────────────────────────────────────

struct OAuthExchangeInfo {
    nonce: String,
    provider: String,
}

fn exchange_oauth_token(agent: &ureq::Agent, nonce: &str, provider: &str) -> Result<String> {
    let url = format!("{}/users/oauth/get_token", DEEPSEEK_AUTH_BASE);
    let body = serde_json::json!({
        "nonce": nonce,
        "provider": provider,
    });

    let resp = agent
        .post(&url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        )
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("Origin", DEEPSEEK_BASE_URL)
        .header("Referer", &format!("{}/sign_in", DEEPSEEK_BASE_URL))
        .send_json(&body)
        .map_err(|e| DeepSeekError::OAuthFailed(format!("token exchange request failed: {}", e)))?;

    let payload: serde_json::Value = resp.into_body().read_json().map_err(|e| {
        DeepSeekError::OAuthFailed(format!("token exchange returned non-JSON: {}", e))
    })?;

    // Try multiple paths to find the token
    let token = payload
        .pointer("/data/biz_data/token")
        .or_else(|| payload.pointer("/data/token"))
        .and_then(|v| v.as_str())
        .or_else(|| {
            payload
                .get("data")
                .and_then(|d| d.get("biz_data"))
                .and_then(|b| b.get("token"))
                .and_then(|t| t.as_str())
        })
        .or_else(|| {
            payload
                .get("data")
                .and_then(|d| d.get("token"))
                .and_then(|t| t.as_str())
        });

    match token {
        Some(t) if !t.is_empty() => Ok(t.to_string()),
        _ => {
            let code = payload
                .pointer("/data/biz_code")
                .or_else(|| payload.pointer("/code"))
                .and_then(|c| c.as_i64())
                .unwrap_or(-1);
            let msg = payload
                .pointer("/data/biz_msg")
                .or_else(|| payload.pointer("/msg"))
                .and_then(|m| m.as_str())
                .unwrap_or("unknown");
            Err(DeepSeekError::OAuthFailed(format!(
                "token exchange failed: code={}, msg={}",
                code, msg
            )))
        }
    }
}

// ── HTML/JS parsing helpers ─────────────────────────────────

fn parse_uuid(html: &str) -> Option<String> {
    // Pattern 1: var fordevtool = "...uuid=XXX"
    let re1 = Regex::new(
        r#"var\s+fordevtool\s*=\s*"https://long\.open\.weixin\.qq\.com/connect/l/qrconnect\?uuid=([^"]+)""#,
    )
    .ok()?;
    if let Some(cap) = re1.captures(html) {
        return cap.get(1).map(|m| m.as_str().to_string());
    }

    // Pattern 2: img src="/connect/qrcode/XXXX"
    let re2 = Regex::new(r"/connect/qrcode/([a-zA-Z0-9]+)").ok()?;
    if let Some(cap) = re2.captures(html) {
        return cap.get(1).map(|m| m.as_str().to_string());
    }

    // Pattern 3: uuid in any JS context
    let re3 = Regex::new(r#"uuid[=:]\s*["']?([a-zA-Z0-9]{10,30})["']?"#).ok()?;
    re3.captures(html)
        .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
}

fn parse_poll_code(body: &str) -> i32 {
    // Primary: window.wx_errcode = NNN
    if let Ok(re) = Regex::new(r"window\.wx_errcode\s*=\s*(\d+)")
        && let Some(cap) = re.captures(body)
        && let Some(m) = cap.get(1)
        && let Ok(code) = m.as_str().parse::<i32>()
    {
        return code;
    }
    // Fallback: detect HTML patterns
    if body.contains("wx_after_scan") && !body.contains("wx_default_tip") {
        return 404;
    }
    if body.contains("扫码成功") || body.contains("扫描成功") {
        return 404;
    }
    if body.contains("wx_after_cancel") {
        return 403;
    }
    408 // Default: still waiting
}

fn parse_wx_code(body: &str) -> Option<String> {
    let re = Regex::new(r#"(?:window\.)?wx_code\s*=\s*['"]?([^'"\s;]+)['"]?"#).ok()?;
    re.captures(body)
        .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
}

fn parse_redirect_uri(body: &str) -> Option<String> {
    let re = Regex::new(r#"window\.wx_redirect_uri\s*=\s*["']?([^"'\s]+)["']?"#).ok()?;
    re.captures(body)
        .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
}

fn parse_oauth_info(
    current_url: &str,
    location: Option<&str>,
    body: &str,
) -> Option<OAuthExchangeInfo> {
    // Strategy 1: from URLs
    let urls: Vec<&str> = location
        .into_iter()
        .chain(std::iter::once(current_url))
        .collect();

    for url in &urls {
        if let Some(info) = parse_oauth_from_url(url) {
            return Some(info);
        }
    }

    // Strategy 2: from body
    let nonce_re = Regex::new(r#"[?&]nonce=([^&"'\s]+)"#).ok()?;
    let nonce_json_re = Regex::new(r#""nonce"\s*:\s*"([^"\s]+)""#).ok()?;
    let provider_re = Regex::new(r#"[?&]provider=([^&"'\s]+)"#).ok()?;
    let provider_json_re = Regex::new(r#""provider"\s*:\s*"([^"\s]+)""#).ok()?;

    let nonce = nonce_re
        .captures(body)
        .and_then(|c| c.get(1))
        .or_else(|| nonce_json_re.captures(body).and_then(|c| c.get(1)))
        .map(|m| m.as_str().to_string());

    let provider = provider_re
        .captures(body)
        .and_then(|c| c.get(1))
        .or_else(|| provider_json_re.captures(body).and_then(|c| c.get(1)))
        .map(|m| m.as_str().to_string());

    if let (Some(nonce), Some(provider)) = (nonce, provider) {
        return Some(OAuthExchangeInfo {
            nonce,
            provider: provider.to_uppercase(),
        });
    }

    None
}

fn parse_oauth_from_url(url: &str) -> Option<OAuthExchangeInfo> {
    let nonce = get_query_param(url, "nonce");
    let provider = get_query_param(url, "provider");
    if let (Some(n), Some(p)) = (nonce, provider) {
        return Some(OAuthExchangeInfo {
            nonce: n,
            provider: p.to_uppercase(),
        });
    }
    None
}

fn get_query_param(url: &str, key: &str) -> Option<String> {
    let pattern = format!(r#"[?&]{}=([^&"'\s]+)"#, regex::escape(key));
    let re = Regex::new(&pattern).ok()?;
    re.captures(url)
        .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
}

fn parse_embedded_token(body: &str) -> Option<String> {
    let re = Regex::new(r#""value"\s*:\s*"([A-Za-z0-9+/=]{40,})""#).ok()?;
    re.captures(body)
        .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
}
