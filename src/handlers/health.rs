use crate::models::Health;
use crate::state::AppState;
use crate::utils::constant_time_compare;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use rovo::response::Json;
use rovo::rovo;

/// Service liveness check
///
/// # Responses
///
/// 200: () - Service is alive
///
/// # Metadata
///
/// @tag health
#[rovo]
pub async fn healthz(State(_state): State<AppState>) -> StatusCode {
    StatusCode::OK
}

/// Service readiness check
///
/// # Responses
///
/// 200: Json<Health> - Redis connection status
/// 401: () - Invalid or missing admin token
/// 503: () - Redis connection failure
///
/// # Metadata
///
/// @tag health
/// @security `admin_token`
/// @security `bearer_auth`
#[rovo]
pub async fn readyz(
    State(mut state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Health>, StatusCode> {
    let token_candidate = headers
        .get("X-Admin-Token")
        .and_then(|h| h.to_str().ok())
        .or_else(|| {
            headers
                .get(header::AUTHORIZATION)
                .and_then(|h| h.to_str().ok())
                .and_then(|auth| auth.strip_prefix("Bearer "))
        });
    let is_authorized =
        token_candidate.is_some_and(|token| constant_time_compare(token, &state.admin_token));

    if !is_authorized {
        return Err(StatusCode::UNAUTHORIZED);
    }

    match redis::cmd("PING")
        .query_async::<String>(&mut state.redis)
        .await
    {
        Ok(pong) if pong == "PONG" => Ok(Json(Health {
            status: "ok".to_string(),
        })),
        Ok(other) => {
            tracing::warn!("Unexpected Redis PING response in /readyz: '{other}'");
            Err(StatusCode::SERVICE_UNAVAILABLE)
        }
        Err(e) => {
            tracing::error!("Redis connection failed during /readyz PING: {e}");
            Err(StatusCode::SERVICE_UNAVAILABLE)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_auth_extraction_accepts_admin_token_header() {
        let expected = "super_admin_secret_token_123";

        let mut headers = HeaderMap::new();
        headers.insert("X-Admin-Token", HeaderValue::from_static(expected));
        let token = headers.get("X-Admin-Token").and_then(|h| h.to_str().ok());
        assert!(token.is_some_and(|t| constant_time_compare(t, expected)));
    }

    #[test]
    fn test_auth_extraction_accepts_bearer_token() {
        let expected = "super_admin_secret_token_123";

        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            format!("Bearer {expected}").parse().unwrap(),
        );
        let token = headers
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|auth| auth.strip_prefix("Bearer "));
        assert!(token.is_some_and(|t| constant_time_compare(t, expected)));
    }
}
