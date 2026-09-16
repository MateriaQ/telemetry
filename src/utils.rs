use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};
use std::net::IpAddr;
use std::time::{SystemTime, UNIX_EPOCH};
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

pub fn current_unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs().cast_signed())
}

pub fn sign_user_hmac(secret: &[u8], user_hash: &str) -> Option<String> {
    if secret.len() < 12 {
        return None;
    }

    let mut mac = HmacSha256::new_from_slice(secret).ok()?;
    mac.update(user_hash.as_bytes());
    let result = mac.finalize().into_bytes();

    Some(hex::encode(result))
}

pub fn verify_user_hmac(secret: &[u8], user_hash: &str, signature_hex: &str) -> bool {
    if secret.len() < 12 {
        return false;
    }

    let Ok(mut mac) = HmacSha256::new_from_slice(secret) else {
        return false;
    };
    mac.update(user_hash.as_bytes());

    let Ok(expected_sig) = hex::decode(signature_hex) else {
        return false;
    };

    mac.verify_slice(&expected_sig).is_ok()
}

pub fn constant_time_compare(a: &str, b: &str) -> bool {
    let hash_a = Sha256::digest(a.as_bytes());
    let hash_b = Sha256::digest(b.as_bytes());
    hash_a.ct_eq(&hash_b).into()
}

pub fn client_ip_from_headers(headers: &axum::http::HeaderMap) -> IpAddr {
    if let Some(cf_ip) = headers
        .get("cf-connecting-ip")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<IpAddr>().ok())
    {
        return cf_ip;
    }
    IpAddr::from([127, 0, 0, 1])
}

pub fn parse_cookie(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    for part in headers
        .get(axum::http::header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
    {
        let (key, value) = part.trim().split_once('=')?;
        if key.trim() == name {
            return Some(value.trim().to_string());
        }
    }
    None
}

pub fn unix_timestamp_to_date(ts: i64) -> String {
    let z = (ts / 86_400) + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = u32::try_from(z - era * 146_097).expect("day-of-era is always within u32 range");
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = i64::from(yoe) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = y + i64::from(m <= 2);
    format!("{year:04}-{m:02}-{d:02}")
}

pub fn get_last_7_days(now: i64) -> Vec<String> {
    let mut days = Vec::with_capacity(7);
    for i in (0..7).rev() {
        let ts = now - i * 86_400;
        days.push(unix_timestamp_to_date(ts));
    }
    days
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_and_verify_hmac() {
        let secret = b"super-secret-hmac-key-longer-than-12-bytes";
        let user_hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

        let signature = sign_user_hmac(secret, user_hash).expect("Signing failed");
        assert_eq!(signature.len(), 64);
        assert!(verify_user_hmac(secret, user_hash, &signature));

        let tampered_hash = "0000000000000000000000000000000000000000000000000000000000000000";
        assert!(!verify_user_hmac(secret, tampered_hash, &signature));
    }

    #[test]
    fn test_constant_time_compare() {
        assert!(constant_time_compare(
            "correct_token_123",
            "correct_token_123"
        ));
        assert!(!constant_time_compare(
            "correct_token_123",
            "wrong_token_12345"
        ));
    }

    #[test]
    fn test_unix_timestamp_to_date() {
        assert_eq!(unix_timestamp_to_date(0), "1970-01-01");
        assert_eq!(unix_timestamp_to_date(86400), "1970-01-02");
        assert_eq!(unix_timestamp_to_date(1_757_946_060), "2025-09-15");
    }

    #[test]
    fn test_get_last_7_days() {
        let days = get_last_7_days(1_757_946_060);
        assert_eq!(days.len(), 7);
        assert_eq!(days[6], "2025-09-15");
        assert_eq!(days[5], "2025-09-14");
    }
}
