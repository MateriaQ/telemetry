use crate::state::AppState;
use axum::extract::State;
use axum::http::header;
use axum::response::Html;
use rovo::rovo;

const CACHE_HEADERS: [(header::HeaderName, &str); 3] = [
    (
        header::CACHE_CONTROL,
        "public, max-age=3600, s-maxage=86400, stale-while-revalidate=604800",
    ),
    (
        header::HeaderName::from_static("cdn-cache-control"),
        "max-age=86400, stale-while-revalidate=604800",
    ),
    (
        header::HeaderName::from_static("cloudflare-cdn-cache-control"),
        "max-age=86400, stale-while-revalidate=604800",
    ),
];

/// Serve the Terms of Service page
///
/// # Responses
///
/// 200: String - Terms of Service page
///
/// # Metadata
///
/// @tag general
#[rovo]
pub async fn terms(
    State(state): State<AppState>,
) -> ([(header::HeaderName, &'static str); 3], Html<String>) {
    (CACHE_HEADERS, Html(state.terms_html))
}

/// Serve the Privacy Policy page
///
/// # Responses
///
/// 200: String - Privacy Policy page
///
/// # Metadata
///
/// @tag general
#[rovo]
pub async fn privacy(
    State(state): State<AppState>,
) -> ([(header::HeaderName, &'static str); 3], Html<String>) {
    (CACHE_HEADERS, Html(state.privacy_html))
}
