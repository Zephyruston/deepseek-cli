use thiserror::Error;

#[derive(Error, Debug)]
pub enum DeepSeekError {
    #[error("Not authenticated. Run `deepseek login` or `deepseek token <value>` first.")]
    NotAuthenticated,

    #[error("Session token expired. Run `deepseek login` to re-authenticate.")]
    TokenExpired,

    #[error("API error {code}: {msg}")]
    ApiError { code: i32, msg: String },

    #[error("API business error {code}: {msg}")]
    ApiBizError { code: i32, msg: String },

    #[error("HTTP request failed: {0}")]
    Http(String),

    #[error("Request timed out")]
    Timeout,

    #[error("Failed to parse API response: {0}")]
    Parse(String),

    #[error("WeChat QR login failed: {0}")]
    QrLogin(String),

    #[error("WeChat poll timed out after {0}s")]
    PollTimeout(u64),

    #[error("OAuth token exchange failed: {0}")]
    OAuthFailed(String),

    #[error("Token storage error: {0}")]
    TokenStorage(String),
}

impl From<ureq::Error> for DeepSeekError {
    fn from(e: ureq::Error) -> Self {
        let msg = e.to_string();
        if msg.contains("timed out") || msg.contains("timeout") {
            DeepSeekError::Timeout
        } else {
            DeepSeekError::Http(msg)
        }
    }
}

pub type Result<T> = std::result::Result<T, DeepSeekError>;
