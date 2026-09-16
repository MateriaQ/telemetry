use crate::app::{CUSTOM_APPS, WEB_APP, find_app, tracks_lifetime};
use crate::constants::{WEB_AUTH_COOKIE, daily_key, lifetime_key, live_key};
use crate::models::PingResponse;
use crate::state::AppState;
use crate::utils::{current_unix_timestamp, unix_timestamp_to_date, verify_user_hmac};
use axum::extract::{RawQuery, State};
use axum::http::{HeaderMap, StatusCode};
use rovo::response::Json;
use rovo::rovo;
use sha2::{Digest, Sha256};
use uuid::Uuid;

fn resolve_identity(headers: &HeaderMap) -> Option<(String, String)> {
    let id_header = headers
        .get("X-User-ID")
        .and_then(|v| v.to_str().ok())
        .map(str::trim);

    if let Some(id) = id_header {
        let sig = headers
            .get("X-User-Signature")
            .and_then(|v| v.to_str().ok())
            .map(str::trim);
        return sig.map(|s| (id.to_string(), s.to_string()));
    }

    let cookie = crate::utils::parse_cookie(headers, WEB_AUTH_COOKIE)?;
    let (id, sig) = cookie.split_once('.')?;
    if Uuid::parse_str(id).is_ok() {
        Some((id.to_string(), sig.to_string()))
    } else {
        None
    }
}

/// Record a presence ping
///
/// # Responses
///
/// 200: Json<PingResponse> - Ping recorded
/// 400: Json<PingResponse> - Missing or invalid user ID, or invalid app ID
/// 401: Json<PingResponse> - Invalid HMAC signature
/// 503: Json<PingResponse> - Redis unavailable
///
/// # Metadata
///
/// @tag telemetry
/// @security `user_id`
/// @security `user_signature`
#[rovo]
pub async fn ping(
    State(state): State<AppState>,
    RawQuery(raw_query): RawQuery,
    headers: HeaderMap,
) -> (StatusCode, Json<PingResponse>) {
    let Some((user_id, sig_header)) = resolve_identity(&headers) else {
        return (StatusCode::BAD_REQUEST, Json(PingResponse::MissingUserId));
    };

    if Uuid::parse_str(&user_id).is_err() {
        return (StatusCode::BAD_REQUEST, Json(PingResponse::InvalidUserId));
    }

    let user_id_clean = user_id.to_lowercase();

    if !verify_user_hmac(&state.hmac_secret, &user_id_clean, &sig_header) {
        tracing::warn!(
            "Rejecting unauthenticated /ping for ID: {}",
            &user_id_clean[..8]
        );
        return (StatusCode::UNAUTHORIZED, Json(PingResponse::Unauthorized));
    }

    let user_hash = hex::encode(Sha256::digest(user_id_clean.as_bytes()));

    let app_candidate = headers
        .get("X-App-ID")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            raw_query.as_ref().and_then(|q| {
                q.split('&').find_map(|pair| {
                    let mut parts = pair.splitn(2, '=');
                    if parts.next()? == "app" {
                        let val = parts.next()?.trim();
                        if val.is_empty() { None } else { Some(val) }
                    } else {
                        None
                    }
                })
            })
        });

    let app = match app_candidate {
        Some(candidate) => match find_app(candidate) {
            Some(app) => app,
            None => {
                return (StatusCode::BAD_REQUEST, Json(PingResponse::InvalidAppId));
            }
        },
        None => CUSTOM_APPS.first().unwrap_or(&WEB_APP),
    };

    let now = current_unix_timestamp();
    let today = unix_timestamp_to_date(now);
    let app_str = app.id;

    let mut redis = state.redis.clone();
    let mut pipe = redis::pipe();
    pipe.atomic();
    pipe.zadd(live_key(app_str), &user_hash, now).ignore();

    let daily = daily_key(app_str, &today);
    pipe.sadd(&daily, &user_hash).ignore();
    pipe.expire(&daily, 9 * 86400).ignore();

    if tracks_lifetime(app_str) {
        pipe.sadd(lifetime_key(app_str), &user_hash).ignore();
    }

    if let Err(e) = pipe.query_async::<()>(&mut redis).await {
        tracing::error!(
            "Redis write failure in /ping pipeline for app '{app_str}', hash {}: {e}",
            &user_hash[..8]
        );
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(PingResponse::ServiceUnavailable),
        );
    }

    tracing::debug!(
        "Recorded authentic ping for app '{app_str}', hash {}",
        &user_hash[..8]
    );
    (StatusCode::OK, Json(PingResponse::Ok))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ping_response_serialization() {
        assert_eq!(
            serde_json::to_string(&PingResponse::Ok).unwrap(),
            "{\"status\":\"ok\"}"
        );
        assert_eq!(
            serde_json::to_string(&PingResponse::InvalidAppId).unwrap(),
            "{\"status\":\"invalid_app_id\"}"
        );
        assert_eq!(
            serde_json::to_string(&PingResponse::MissingUserId).unwrap(),
            "{\"status\":\"missing_user_id\"}"
        );
        assert_eq!(
            serde_json::to_string(&PingResponse::Unauthorized).unwrap(),
            "{\"status\":\"unauthorized\"}"
        );
    }
}
