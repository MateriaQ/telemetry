use crate::constants::{IDENTITY_CAP_PER_IP, IDENTITY_CAP_TTL_SECS};
use redis::{RedisResult, Script};
use std::time::{SystemTime, UNIX_EPOCH};

const TOKEN_BUCKET_SCRIPT: &str = r"
local key = KEYS[1]
local capacity = tonumber(ARGV[1])
local refill_rate = tonumber(ARGV[2])
local refill_interval = tonumber(ARGV[3])
local now = tonumber(ARGV[4])

local bucket = redis.call('HMGET', key, 'tokens', 'last_refill')
local tokens = tonumber(bucket[1])
local last_refill = tonumber(bucket[2])

if tokens == nil then
    tokens = capacity
    last_refill = now
end

local time_passed = now - last_refill
local refills = math.floor(time_passed / refill_interval)

if refills > 0 then
    tokens = math.min(capacity, tokens + (refills * refill_rate))
    last_refill = last_refill + (refills * refill_interval)
end

local allowed = 0
if tokens >= 1 then
    tokens = tokens - 1
    allowed = 1
end

-- 1-hour TTL so abandoned IP keys clean up automatically
redis.call('HSET', key, 'tokens', tokens, 'last_refill', last_refill)
redis.call('EXPIRE', key, 3600)

return {allowed, math.floor(tokens)}
";

const IDENTITY_CAP_SCRIPT: &str = r"
local key = KEYS[1]
local cap = tonumber(ARGV[1])
local user_id = ARGV[2]
local ttl = tonumber(ARGV[3])

-- Returning identities never consume budget (idempotent)
local present = redis.call('SISMEMBER', key, user_id)
if present == 1 then
    return 1
end

local current = redis.call('SCARD', key)
if current >= cap then
    return 0
end

redis.call('SADD', key, user_id)
redis.call('EXPIRE', key, ttl)
return 1
";

#[derive(Clone)]
pub struct IdentityCap {
    cap: usize,
    script: Script,
}

impl IdentityCap {
    pub fn new(cap: usize) -> Self {
        Self {
            cap,
            script: Script::new(IDENTITY_CAP_SCRIPT),
        }
    }

    pub async fn allow(
        &self,
        con: &mut redis::aio::ConnectionManager,
        key: &str,
        user_id: &str,
    ) -> RedisResult<bool> {
        let result: i64 = self
            .script
            .key(key)
            .arg(self.cap)
            .arg(user_id)
            .arg(IDENTITY_CAP_TTL_SECS)
            .invoke_async(con)
            .await?;
        Ok(result == 1)
    }
}

impl Default for IdentityCap {
    fn default() -> Self {
        Self::new(IDENTITY_CAP_PER_IP)
    }
}

#[derive(Debug, Clone)]
pub struct RateLimitResult {
    pub allowed: bool,
    pub remaining: u64,
}

static TOKEN_BUCKET_LUA: std::sync::LazyLock<Script> =
    std::sync::LazyLock::new(|| Script::new(TOKEN_BUCKET_SCRIPT));

#[derive(Clone, Copy)]
pub struct TokenBucket {
    pub capacity: i64,
    pub refill_rate: f64,
    pub refill_interval: u64,
}

impl TokenBucket {
    #[inline]
    pub const fn new(capacity: i64, refill_rate: f64, refill_interval: u64) -> Self {
        Self {
            capacity,
            refill_rate,
            refill_interval,
        }
    }

    pub async fn allow(
        &self,
        con: &mut redis::aio::ConnectionManager,
        key: &str,
    ) -> RedisResult<RateLimitResult> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs_f64();

        let result: Vec<redis::Value> = TOKEN_BUCKET_LUA
            .key(key)
            .arg(self.capacity)
            .arg(self.refill_rate)
            .arg(self.refill_interval)
            .arg(now)
            .invoke_async(con)
            .await?;

        let allowed = match result.first() {
            Some(redis::Value::Int(v)) => *v == 1,
            _ => false,
        };

        let remaining = match result.get(1) {
            Some(redis::Value::Int(v)) => u64::try_from(*v).unwrap_or(0),
            Some(redis::Value::BulkString(v)) => {
                String::from_utf8_lossy(v).parse::<u64>().unwrap_or(0)
            }
            _ => 0,
        };

        Ok(RateLimitResult { allowed, remaining })
    }
}
