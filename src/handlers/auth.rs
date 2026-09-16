use crate::app::{find_app, is_web};
use crate::constants::{WEB_AUTH_COOKIE, WEB_AUTH_COOKIE_MAX_AGE_SECS, identity_key};
use crate::models::AuthResponse;
use crate::state::AppState;
use crate::utils::{client_ip_from_headers, sign_user_hmac};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use rovo::response::Json;
use rovo::rovo;
use uuid::Uuid;

const MIN_IDENTIFIER_LEN: usize = 16;
const MAX_IDENTIFIER_LEN: usize = 128;

fn normalize_spotify_id(id: &str) -> String {
    let trimmed = id.trim();
    let stripped = trimmed
        .strip_prefix("spotify:user:")
        .or_else(|| trimmed.strip_prefix("Spotify:User:"))
        .or_else(|| trimmed.strip_prefix("spotify:User:"))
        .unwrap_or(trimmed);
    stripped.to_lowercase()
}

fn is_valid_spotify_id(id: &str) -> bool {
    let stripped = id
        .strip_prefix("spotify:user:")
        .or_else(|| id.strip_prefix("Spotify:User:"))
        .or_else(|| id.strip_prefix("spotify:User:"))
        .unwrap_or(id);
    stripped.len() >= MIN_IDENTIFIER_LEN
        && stripped.len() <= MAX_IDENTIFIER_LEN
        && stripped.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '@' || c == ':'
        })
}

fn caller_ip(headers: &HeaderMap) -> String {
    client_ip_from_headers(headers).to_string()
}

fn parse_auth_cookie(headers: &HeaderMap, secret: &[u8]) -> Option<String> {
    let value = crate::utils::parse_cookie(headers, WEB_AUTH_COOKIE)?;
    let (user_id, signature) = value.split_once('.')?;
    let parsed = Uuid::parse_str(user_id).ok()?;
    let user_id_clean = parsed.to_string();
    if crate::utils::verify_user_hmac(secret, &user_id_clean, signature) {
        Some(user_id_clean)
    } else {
        None
    }
}

fn build_auth_cookie(user_id: &str, signature: &str, secure: bool) -> String {
    let secure_attr = if secure { "; Secure" } else { "" };
    format!(
        "{WEB_AUTH_COOKIE}={user_id}.{signature}; Path=/; Max-Age={WEB_AUTH_COOKIE_MAX_AGE_SECS}; HttpOnly; SameSite=Lax{secure_attr}"
    )
}

/// Mint an auth token
///
/// # Responses
///
/// 200: Json<AuthResponse> - User ID and HMAC signature
/// 400: () - Invalid Spotify ID or missing app
/// 429: () - Daily identity budget exceeded
/// 500: () - Signing failure
/// 503: () - Redis unavailable
///
/// # Metadata
///
/// @tag auth
/// @security `spotify_id`
/// @security `app_id`
#[rovo]
pub async fn get_ping_token(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Json<AuthResponse>), StatusCode> {
    let spotify_id = headers
        .get("X-Spotify-Id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim);

    let app_config = headers
        .get("X-App-ID")
        .and_then(|v| v.to_str().ok())
        .and_then(find_app);

    let client_id = headers
        .get("X-Client-ID")
        .and_then(|v| v.to_str().ok())
        .map(str::trim);

    let is_web_client = app_config.is_some_and(|a| is_web(a.id));
    let mut is_returning_web_user = false;

    let user_id = if let Some(sp_id) = spotify_id {
        if !is_valid_spotify_id(sp_id) {
            return Err(StatusCode::BAD_REQUEST);
        }
        let normalized = normalize_spotify_id(sp_id);
        Uuid::new_v5(&Uuid::NAMESPACE_OID, normalized.as_bytes()).to_string()
    } else if is_web_client {
        if let Some(existing) = parse_auth_cookie(&headers, &state.hmac_secret) {
            is_returning_web_user = true;
            existing
        } else if let Some(cid) = client_id
            && let Ok(parsed_uuid) = Uuid::parse_str(cid)
        {
            parsed_uuid.to_string()
        } else {
            Uuid::new_v4().to_string()
        }
    } else if let Some(cid) = client_id
        && let Ok(parsed_uuid) = Uuid::parse_str(cid)
    {
        parsed_uuid.to_string()
    } else {
        return Err(StatusCode::BAD_REQUEST);
    };

    if !is_returning_web_user {
        let ip = caller_ip(&headers);
        let mut redis = state.redis.clone();
        match state
            .identity_cap
            .allow(&mut redis, &identity_key(&ip), &user_id)
            .await
        {
            Ok(true) => {}
            Ok(false) => {
                tracing::warn!("Rejecting identity mint for IP {ip}: daily budget exceeded");
                return Err(StatusCode::TOO_MANY_REQUESTS);
            }
            Err(e) => {
                tracing::error!("Identity cap Redis failure for IP {ip}: {e}");
                return Err(StatusCode::SERVICE_UNAVAILABLE);
            }
        }
    }

    let signature =
        sign_user_hmac(&state.hmac_secret, &user_id).ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut response_headers = HeaderMap::new();
    if is_web_client {
        response_headers.insert(
            header::SET_COOKIE,
            build_auth_cookie(&user_id, &signature, state.cookie_secure)
                .parse()
                .unwrap(),
        );
    }

    Ok((response_headers, Json(AuthResponse { user_id, signature })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::verify_user_hmac;

    #[test]
    fn test_spotify_id_length_validation() {
        assert!(!is_valid_spotify_id(""));
        assert!(!is_valid_spotify_id("short_id"));
        assert!(is_valid_spotify_id("31xduo7o66xv3gq4st5zqvjssu7a"));
        assert!(is_valid_spotify_id(
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        ));
        assert!(!is_valid_spotify_id("31xduo7o66xv3gq4st5zqvjssu7a\n"));
    }

    #[test]
    fn test_deterministic_user_id_generation() {
        let spotify_id = "31xduo7o66xv3gq4st5zqvjssu7a";
        let id1 = Uuid::new_v5(&Uuid::NAMESPACE_OID, spotify_id.as_bytes()).to_string();
        let id2 = Uuid::new_v5(&Uuid::NAMESPACE_OID, spotify_id.as_bytes()).to_string();

        assert_eq!(id1, id2);
        assert!(Uuid::parse_str(&id1).is_ok());
    }

    #[test]
    fn test_auth_response_signature_validity() {
        let hmac_secret = b"secure-test-secret-at-least-12-bytes";
        let spotify_id = "31xduo7o66xv3gq4st5zqvjssu7a";
        let user_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, spotify_id.as_bytes()).to_string();

        let signature = sign_user_hmac(hmac_secret, &user_id).unwrap();
        assert!(verify_user_hmac(hmac_secret, &user_id, &signature));
    }

    #[test]
    fn test_auth_cookie_roundtrip() {
        let secret = b"super-secure-secret-longer-than-12-bytes";
        let user_id = Uuid::new_v4().to_string();
        let signature = sign_user_hmac(secret, &user_id).unwrap();
        let cookie = build_auth_cookie(&user_id, &signature, true);

        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));
        assert!(cookie.contains("Secure"));
        assert!(cookie.contains(&format!("{WEB_AUTH_COOKIE}={user_id}.{signature};")));

        // Valid signature passes
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, cookie.parse().unwrap());
        assert_eq!(
            parse_auth_cookie(&headers, secret).as_deref(),
            Some(user_id.as_str())
        );

        // Forged signature is rejected
        let forged_cookie = build_auth_cookie(&user_id, &"00".repeat(32), true);
        let mut forged_headers = HeaderMap::new();
        forged_headers.insert(header::COOKIE, forged_cookie.parse().unwrap());
        assert_eq!(parse_auth_cookie(&forged_headers, secret), None);
    }

    #[test]
    fn test_cross_device_normalization() {
        let sp1 = "31xduo7o66xv3gq4st5zqvjssu7a";
        let sp2 = "Spotify:User:31Xduo7o66xv3gq4st5zqvjssu7a";
        let sp3 = "spotify:user:31xduo7o66xv3gq4st5zqvjssu7a ";

        let norm1 = normalize_spotify_id(sp1);
        let norm2 = normalize_spotify_id(sp2);
        let norm3 = normalize_spotify_id(sp3);

        assert_eq!(norm1, norm2);
        assert_eq!(norm2, norm3);

        let id1 = Uuid::new_v5(&Uuid::NAMESPACE_OID, norm1.as_bytes()).to_string();
        let id2 = Uuid::new_v5(&Uuid::NAMESPACE_OID, norm2.as_bytes()).to_string();
        let id3 = Uuid::new_v5(&Uuid::NAMESPACE_OID, norm3.as_bytes()).to_string();

        assert_eq!(id1, id2);
        assert_eq!(id2, id3);
    }
}
