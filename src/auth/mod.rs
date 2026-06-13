use crate::Result;
use crate::display;
use crate::error::DeepSeekError;

pub mod storage;
pub mod wechat;

/// Orchestrates the full WeChat QR login flow and token management.
pub struct AuthManager {
    agent: ureq::Agent,
}

impl Default for AuthManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthManager {
    pub fn new() -> Self {
        Self {
            agent: ureq::Agent::new_with_config(
                ureq::Agent::config_builder()
                    .timeout_global(Some(std::time::Duration::from_secs(35)))
                    .http_status_as_error(false)
                    .build(),
            ),
        }
    }

    /// Full WeChat QR login: fetch QR, display, poll, exchange, store, validate.
    pub fn login_interactive(&mut self) -> Result<()> {
        // 1. Fetch QR code
        display::show_login_status("fetching");
        let session = wechat::fetch_qr_code(&self.agent)?;

        // 2. Show QR code
        display::show_qr_code(&session.qr_content);
        display::show_login_status("waiting");

        // 3. Poll for scan
        let wx_code = match wechat::poll_for_login(&self.agent, &session.uuid) {
            Ok(code) => {
                display::show_login_status("confirmed");
                code
            }
            Err(e) => {
                if matches!(e, DeepSeekError::PollTimeout(_)) {
                    display::show_login_status("expired");
                } else {
                    display::show_login_status("error");
                }
                return Err(e);
            }
        };

        // 4. Handle OAuth callback + exchange for token
        let token = wechat::handle_oauth_callback(&self.agent, &wx_code)?;

        // 5. Store token
        storage::store_token(&token)?;

        // 6. Validate
        eprint!("Validating token... ");
        if storage::validate_token(&token) {
            display::show_login_status("success");
        } else {
            eprintln!("warning: token validation failed, but it has been stored");
        }

        Ok(())
    }

    /// Set token manually (from user input string).
    pub fn set_token(token: &str) -> Result<()> {
        // Parse JSON blob if present: {"value":"...","__version":"0"}
        let value = if token.trim().starts_with('{') {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(token.trim()) {
                parsed["value"].as_str().unwrap_or(token.trim()).to_string()
            } else {
                token.trim().to_string()
            }
        } else {
            token.trim().to_string()
        };

        storage::store_token(&value)?;
        if storage::validate_token(&value) {
            println!("✓ Token validated and stored.");
        } else {
            println!("⚠ Token stored but validation failed. It may be expired.");
        }
        Ok(())
    }

    /// Logout: clear stored token.
    pub fn logout() -> Result<()> {
        storage::clear_token()?;
        println!("Logged out. Token cleared.");
        Ok(())
    }
}
