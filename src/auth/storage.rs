use serde::{Deserialize, Serialize};

use crate::Result;
use crate::error::DeepSeekError;

const APP_NAME: &str = "deepseek-cli";

#[derive(Serialize, Deserialize, Default)]
struct Config {
    token: String,
}

/// Store the session token via confy (XDG config dir, platform-appropriate path).
pub fn store_token(token: &str) -> Result<()> {
    let cfg = Config {
        token: token.trim().to_string(),
    };
    confy::store(APP_NAME, Some("config"), &cfg)
        .map_err(|e| DeepSeekError::TokenStorage(format!("confy store failed: {}", e)))?;
    // Restrict permissions on Unix
    #[cfg(unix)]
    if let Ok(path) = confy::get_configuration_file_path(APP_NAME, Some("config")) {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Retrieve the stored session token.
pub fn get_token() -> Result<String> {
    let cfg: Config = confy::load(APP_NAME, Some("config"))
        .map_err(|e| DeepSeekError::TokenStorage(format!("confy load failed: {}", e)))?;
    if cfg.token.is_empty() {
        Err(DeepSeekError::NotAuthenticated)
    } else {
        Ok(cfg.token)
    }
}

/// Clear the stored token.
pub fn clear_token() -> Result<()> {
    store_token("")?;
    // Also remove the file for cleanliness
    if let Ok(path) = confy::get_configuration_file_path(APP_NAME, Some("config")) {
        let _ = std::fs::remove_file(&path);
    }
    Ok(())
}

/// Validate a token by making a quick API call.
/// Returns true if the token yields code === 0.
pub fn validate_token(token: &str) -> bool {
    let url = format!(
        "{}/users/get_user_summary",
        crate::constants::DEEPSEEK_API_BASE
    );

    match ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(10)))
            .build(),
    )
    .get(&url)
    .header("Authorization", &format!("Bearer {}", token))
    .header("Accept", "application/json")
    .call()
    {
        Ok(resp) => resp
            .into_body()
            .read_json::<serde_json::Value>()
            .map(|json| json.get("code").and_then(|c| c.as_i64()) == Some(0))
            .unwrap_or(false),
        Err(_) => false,
    }
}
