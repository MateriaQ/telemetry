use crate::constants::{
    AUTH_BURST, AUTH_REFILL_INTERVAL, AUTH_REFILL_RATE, HEALTHZ_BURST, HEALTHZ_REFILL_INTERVAL,
    HEALTHZ_REFILL_RATE, PING_BURST, PING_REFILL_INTERVAL, PING_REFILL_RATE, RATE_LIMIT_PREFIX,
    READYZ_BURST, READYZ_REFILL_INTERVAL, READYZ_REFILL_RATE, USERS_BURST, USERS_REFILL_INTERVAL,
    USERS_REFILL_RATE,
};
use crate::state::AppState;
use crate::token_bucket::TokenBucket;
use crate::utils::client_ip_from_headers;
use axum::{
    extract::{Request, State},
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::net::{IpAddr, SocketAddr};

fn extract_client_ip(req: &Request) -> IpAddr {
    let header_ip = client_ip_from_headers(req.headers());
    if header_ip != IpAddr::from([127, 0, 0, 1]) {
        return header_ip;
    }

    if let Some(connect_info) = req
        .extensions()
        .get::<axum::extract::ConnectInfo<SocketAddr>>()
    {
        return connect_info.0.ip();
    }

    header_ip
}

async fn apply_token_bucket(
    mut state: AppState,
    limiter: TokenBucket,
    endpoint_name: &'static str,
    req: Request,
    next: Next,
) -> Response {
    let ip = extract_client_ip(&req);
    let key = format!("{RATE_LIMIT_PREFIX}:{endpoint_name}:{ip}");

    match limiter.allow(&mut state.redis, &key).await {
        Ok(result) => {
            if result.allowed {
                let mut response = next.run(req).await;
                response
                    .headers_mut()
                    .insert("X-RateLimit-Limit", HeaderValue::from(limiter.capacity));
                response
                    .headers_mut()
                    .insert("X-RateLimit-Remaining", HeaderValue::from(result.remaining));
                response
            } else {
                let error_payload = serde_json::to_string(&crate::models::ApiError {
                    error: "Too many requests. Rate limit exceeded.".to_string(),
                })
                .unwrap_or_else(|_| {
                    "{\"error\":\"Too many requests. Rate limit exceeded.\"}".to_string()
                });

                let mut response = (StatusCode::TOO_MANY_REQUESTS, error_payload).into_response();
                response.headers_mut().insert(
                    axum::http::header::CONTENT_TYPE,
                    HeaderValue::from_static("application/json"),
                );
                response
                    .headers_mut()
                    .insert("X-RateLimit-Limit", HeaderValue::from(limiter.capacity));
                response
                    .headers_mut()
                    .insert("X-RateLimit-Remaining", HeaderValue::from(0));
                response
                    .headers_mut()
                    .insert("Retry-After", HeaderValue::from(limiter.refill_interval));
                response
            }
        }
        Err(e) => {
            tracing::error!("Token bucket Redis error on /{endpoint_name}: {e}");
            next.run(req).await
        }
    }
}

pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let ip = extract_client_ip(&req);
    if !req.headers().contains_key("cf-connecting-ip")
        && let Ok(val) = ip.to_string().parse()
    {
        req.headers_mut().insert("cf-connecting-ip", val);
    }

    let raw_path = req.uri().path();
    let path = raw_path.strip_suffix('/').unwrap_or(raw_path);

    if path == "/get-token" {
        let limiter = TokenBucket::new(AUTH_BURST, AUTH_REFILL_RATE, AUTH_REFILL_INTERVAL);
        apply_token_bucket(state, limiter, "auth", req, next).await
    } else if path == "/ping" {
        let limiter = TokenBucket::new(PING_BURST, PING_REFILL_RATE, PING_REFILL_INTERVAL);
        apply_token_bucket(state, limiter, "ping", req, next).await
    } else if path == "/users" || path.starts_with("/users/") {
        let limiter = TokenBucket::new(USERS_BURST, USERS_REFILL_RATE, USERS_REFILL_INTERVAL);
        apply_token_bucket(state, limiter, "users", req, next).await
    } else if path == "/readyz" {
        let limiter = TokenBucket::new(READYZ_BURST, READYZ_REFILL_RATE, READYZ_REFILL_INTERVAL);
        apply_token_bucket(state, limiter, "readyz", req, next).await
    } else if path == "/healthz" {
        let limiter = TokenBucket::new(HEALTHZ_BURST, HEALTHZ_REFILL_RATE, HEALTHZ_REFILL_INTERVAL);
        apply_token_bucket(state, limiter, "healthz", req, next).await
    } else {
        next.run(req).await
    }
}
