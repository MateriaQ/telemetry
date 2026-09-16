use crate::state::AppState;
use axum::extract::State;
use axum::http::header;
use axum::response::Html;
use rovo::rovo;

const CACHE_HEADERS: [(header::HeaderName, &str); 3] = [
    (
        header::CACHE_CONTROL,
        "public, max-age=60, s-maxage=3600, stale-while-revalidate=86400",
    ),
    (
        header::HeaderName::from_static("cdn-cache-control"),
        "max-age=3600, stale-while-revalidate=86400",
    ),
    (
        header::HeaderName::from_static("cloudflare-cdn-cache-control"),
        "max-age=3600, stale-while-revalidate=86400",
    ),
];

/// Serve the landing page
///
/// # Responses
///
/// 200: String - Static landing page
///
/// # Metadata
///
/// @tag general
#[rovo]
pub async fn root(
    State(state): State<AppState>,
) -> ([(header::HeaderName, &'static str); 3], Html<String>) {
    (CACHE_HEADERS, Html(state.index_html))
}
