pub const DEEPSEEK_BASE_URL: &str = "https://platform.deepseek.com";
pub const DEEPSEEK_API_BASE: &str = "https://platform.deepseek.com/api/v0";
pub const DEEPSEEK_AUTH_BASE: &str = "https://platform.deepseek.com/auth-api/v0";

pub const USAGE_BY_KEY_AMOUNT_PATH: &str = "/usage/by_api_key/amount";
pub const USAGE_BY_KEY_COST_PATH: &str = "/usage/by_api_key/cost";
pub const API_KEYS_PATH: &str = "/users/get_api_keys";

/// Browser-like User-Agent to avoid WAF blocking on newer endpoints.
pub const BROWSER_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

pub const WECHAT_OPEN_BASE: &str = "https://open.weixin.qq.com";
pub const WECHAT_LONG_POLL_BASE: &str = "https://long.open.weixin.qq.com";
pub const WECHAT_APP_ID: &str = "wx335255e1b73f9e52";

pub const POLL_INTERVAL_MS: u64 = 2000;
pub const POLL_FAST_INTERVAL_MS: u64 = 100;
pub const POLL_MAX_DURATION_SECS: u64 = 300;

pub const API_TIMEOUT_SECS: u64 = 15;
pub const OAUTH_REQUEST_TIMEOUT_SECS: u64 = 35;

pub const TOKEN_EXPIRED_CODE: i32 = 40002;
pub const API_SUCCESS_CODE: i32 = 0;
