/// Seconds a presence ping keeps an entry "live" in Redis (4 minutes).
pub const LIVE_WINDOW_SECS: i64 = 240;

/// Seconds between background sweeps of stale live entries (5 minutes).
pub const SWEEP_INTERVAL_SECS: u64 = 300;

/// Key prefix shared by token-bucket and identity-cap rate limiters.
pub const RATE_LIMIT_PREFIX: &str = "rate";

/// Name of the web dashboard auth cookie.
pub const WEB_AUTH_COOKIE: &str = "materiaq_web_auth";

/// Lifetime of the web auth cookie (1 year).
pub const WEB_AUTH_COOKIE_MAX_AGE_SECS: u64 = 31_536_000;

/// Default HTTP port (8080).
pub const DEFAULT_PORT: u16 = 8080;

/// Default HTTP request timeout (15 seconds).
pub const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 15;

/// Default HTTP request body limit (64 KB).
pub const DEFAULT_BODY_LIMIT_BYTES: usize = 64 * 1024;

/// Default maximum distinct identities that may be minted per IP per day (100).
pub const DEFAULT_IDENTITY_CAP_PER_IP: usize = 100;
pub const IDENTITY_CAP_PER_IP: usize = DEFAULT_IDENTITY_CAP_PER_IP;

/// TTL for the per-IP identity cap set (24 hours).
pub const IDENTITY_CAP_TTL_SECS: u64 = 86_400;

// Rate Limit Token Bucket Parameters: (Capacity, Refill Rate, Interval Seconds)
/// /get-token: Burst 60, refills 2 tokens every 1s (120/min sustained).
pub const AUTH_BURST: i64 = 60;
pub const AUTH_REFILL_RATE: f64 = 2.0;
pub const AUTH_REFILL_INTERVAL: u64 = 1;

/// /ping: Burst 180, refills 3 tokens every 1s (180/min sustained).
pub const PING_BURST: i64 = 180;
pub const PING_REFILL_RATE: f64 = 3.0;
pub const PING_REFILL_INTERVAL: u64 = 1;

/// /users: Burst 240, refills 4 tokens every 1s (240/min sustained).
pub const USERS_BURST: i64 = 240;
pub const USERS_REFILL_RATE: f64 = 4.0;
pub const USERS_REFILL_INTERVAL: u64 = 1;

/// /readyz: Burst 60, refills 2 tokens every 1s (120/min sustained).
pub const READYZ_BURST: i64 = 60;
pub const READYZ_REFILL_RATE: f64 = 2.0;
pub const READYZ_REFILL_INTERVAL: u64 = 1;

/// /healthz: Burst 300, refills 10 tokens every 1s (600/min sustained).
pub const HEALTHZ_BURST: i64 = 300;
pub const HEALTHZ_REFILL_RATE: f64 = 10.0;
pub const HEALTHZ_REFILL_INTERVAL: u64 = 1;

#[inline]
pub fn live_key(app_id: &str) -> String {
    format!("app:{app_id}:live")
}

#[inline]
pub fn lifetime_key(app_id: &str) -> String {
    format!("app:{app_id}:lifetime")
}

#[inline]
pub fn daily_key(app_id: &str, date_str: &str) -> String {
    format!("app:{app_id}:unique:{date_str}")
}

#[inline]
pub fn identity_key(ip: &str) -> String {
    format!("rate:ident:{ip}")
}
