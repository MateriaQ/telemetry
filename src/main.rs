mod app;
mod constants;
mod handlers;
mod models;
mod rate_limit;
mod state;
mod sweeper;
mod templates;
mod token_bucket;
mod utils;

use axum::ServiceExt;
use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::middleware;
use dotenvy::dotenv;
use indexmap::IndexMap;
use rovo::Router;
use rovo::aide::openapi::{ApiKeyLocation, Info, OpenApi, ReferenceOr, SecurityScheme};
use rovo::routing::{get, post};
use std::net::SocketAddr;
use std::time::Duration;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tower_http::normalize_path::NormalizePathLayer;
use tower_http::services::ServeDir;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::handlers::{
    auth::get_ping_token,
    health::{healthz, readyz},
    legal::{privacy, terms},
    ping::ping,
    root::root,
    users::{app_live, users, users_by_app, users_live},
};
use crate::rate_limit::rate_limit_middleware;
use crate::state::AppState;
use crate::sweeper::start_sweeper;
use crate::token_bucket::IdentityCap;

struct ServerEnv {
    redis_url: String,
    admin_token: String,
    hmac_secret: Vec<u8>,
    cookie_secure: bool,
    docs_enabled: bool,
    identity_cap_per_ip: usize,
    port: u16,
    request_timeout_secs: u64,
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "telemetry=info,tower_http=info,redis=info".into()),
        )
        .with(tracing_subscriber::fmt::layer().without_time())
        .init();

    tracing::info!("Starting up MateriaQ Telemetry Server...");

    let env = load_env();
    let redis = connect_redis(&env.redis_url).await;

    start_sweeper(redis.clone());

    let index_html =
        crate::templates::render_index().expect("Failed to render index template with Askama");
    let terms_html =
        crate::templates::render_terms().expect("Failed to render terms template with Askama");
    let privacy_html =
        crate::templates::render_privacy().expect("Failed to render privacy template with Askama");

    let state = AppState {
        redis,
        admin_token: env.admin_token,
        hmac_secret: env.hmac_secret,
        index_html,
        terms_html,
        privacy_html,
        cookie_secure: env.cookie_secure,
        identity_cap: IdentityCap::new(env.identity_cap_per_ip),
    };

    let api = if env.docs_enabled {
        tracing::info!("OpenAPI docs enabled");
        Some(build_openapi())
    } else {
        None
    };

    let api_routes = build_api_routes(api, state.clone());
    let app = mount_routes(api_routes);
    let app = apply_layers(app, state, env.request_timeout_secs);
    let app = ServiceBuilder::new()
        .layer(NormalizePathLayer::trim_trailing_slash())
        .service(app);

    let port = env.port;
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind to {addr}: {e}"));

    tracing::info!("Server running on http://{addr}");

    axum::serve(
        listener,
        ServiceExt::<axum::extract::Request>::into_make_service_with_connect_info::<SocketAddr>(
            app,
        ),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .unwrap();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("Shutdown signal received, draining connections...");
}

fn load_env() -> ServerEnv {
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL environment variable must be set");
    let admin_token =
        std::env::var("ADMIN_TOKEN").expect("ADMIN_TOKEN environment variable must be set");
    let hmac_secret =
        std::env::var("HMAC_SECRET").expect("HMAC_SECRET environment variable must be set");

    let trimmed_admin = admin_token.trim();
    assert!(
        trimmed_admin.len() >= 12,
        "ADMIN_TOKEN must be at least 12 characters long"
    );
    let admin_token = trimmed_admin.to_string();

    let trimmed_hmac = hmac_secret.trim();
    assert!(
        trimmed_hmac.len() >= 12,
        "HMAC_SECRET must be at least 12 characters long"
    );
    let hmac_secret = trimmed_hmac.as_bytes().to_vec();

    let cookie_secure = std::env::var("COOKIE_SECURE").map_or(true, |v| v.trim() == "true");

    let docs_enabled = std::env::var("ENABLE_DOCS").is_ok_and(|v| v.trim() == "true");

    let identity_cap_per_ip: usize = std::env::var("IDENTITY_CAP_PER_IP")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(crate::constants::DEFAULT_IDENTITY_CAP_PER_IP);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(crate::constants::DEFAULT_PORT);

    let request_timeout_secs: u64 = std::env::var("REQUEST_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(crate::constants::DEFAULT_REQUEST_TIMEOUT_SECS);

    ServerEnv {
        redis_url,
        admin_token,
        hmac_secret,
        cookie_secure,
        docs_enabled,
        identity_cap_per_ip,
        port,
        request_timeout_secs,
    }
}

async fn connect_redis(url: &str) -> redis::aio::ConnectionManager {
    let client =
        redis::Client::open(url).unwrap_or_else(|e| panic!("Invalid REDIS_URL '{url}': {e}"));
    let mut conn = match client.get_connection_manager().await {
        Ok(conn) => conn,
        Err(e) => panic!("Failed to establish Redis ConnectionManager: {e}"),
    };

    match redis::cmd("PING").query_async::<String>(&mut conn).await {
        Ok(_) => tracing::info!("Successfully connected to Redis and received PONG"),
        Err(e) => panic!("Connected to Redis client, but initial PING failed: {e}"),
    }

    conn
}

fn build_openapi() -> OpenApi {
    let mut api = OpenApi {
        info: Info {
            title: "MateriaQ Telemetry API".to_string(),
            description: Some("Privacy-preserving telemetry and live presence API".to_string()),
            version: env!("CARGO_PKG_VERSION").to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

    let components = api.components.get_or_insert_default();
    let mut add_api_key = |name: &str, header: &'static str, description: &str| {
        components.security_schemes.insert(
            name.to_string(),
            ReferenceOr::Item(SecurityScheme::ApiKey {
                name: header.to_string(),
                location: ApiKeyLocation::Header,
                description: Some(description.to_string()),
                extensions: IndexMap::default(),
            }),
        );
    };

    add_api_key(
        "admin_token",
        "X-Admin-Token",
        "Administrative access token",
    );
    add_api_key(
        "spotify_id",
        "X-Spotify-Id",
        "Spotify user identifier or hash",
    );
    add_api_key(
        "app_id",
        "X-App-ID",
        "Target application identifier (e.g. 'lyrics', 'theme', or 'web')",
    );
    add_api_key("user_id", "X-User-ID", "Authenticated client UUID");
    add_api_key(
        "user_signature",
        "X-User-Signature",
        "HMAC signature corresponding to user ID",
    );

    components.security_schemes.insert(
        "bearer_auth".to_string(),
        ReferenceOr::Item(SecurityScheme::Http {
            scheme: "bearer".to_string(),
            bearer_format: None,
            description: Some("Bearer token in Authorization header".to_string()),
            extensions: IndexMap::default(),
        }),
    );

    api
}

fn build_api_routes(api: Option<OpenApi>, state: AppState) -> axum::Router {
    let router = Router::<AppState>::new()
        .route("/", get(root))
        .route("/terms", get(terms))
        .route("/privacy", get(privacy))
        .route("/healthz", get(healthz))
        .route("/ping", get(ping).post(ping))
        .route("/get-token", post(get_ping_token))
        .route("/users", get(users))
        .route("/users/live", get(users_live))
        .route("/users/{app}", get(users_by_app))
        .route("/users/{app}/live", get(app_live))
        .route("/readyz", get(readyz));

    match api {
        Some(api) => router.with_oas(api).with_swagger("/docs"),
        None => router,
    }
    .with_state(state)
    .finish()
}

async fn static_cache_headers(mut res: axum::response::Response) -> axum::response::Response {
    if res.status().is_success() {
        res.headers_mut().insert(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static(
                "public, max-age=86400, s-maxage=604800, stale-while-revalidate=86400",
            ),
        );
        res.headers_mut().insert(
            axum::http::HeaderName::from_static("cdn-cache-control"),
            axum::http::HeaderValue::from_static("max-age=604800, stale-while-revalidate=86400"),
        );
        res.headers_mut().insert(
            axum::http::HeaderName::from_static("cloudflare-cdn-cache-control"),
            axum::http::HeaderValue::from_static("max-age=604800, stale-while-revalidate=86400"),
        );
    }
    res
}

fn mount_routes(api_routes: axum::Router) -> axum::Router {
    let static_router = axum::Router::new()
        .fallback_service(ServeDir::new("public"))
        .layer(middleware::map_response(static_cache_headers));
    api_routes.fallback_service(static_router)
}

async fn security_headers_middleware(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let mut res = next.run(req).await;
    let headers = res.headers_mut();
    headers.insert(
        axum::http::header::X_CONTENT_TYPE_OPTIONS,
        axum::http::HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        axum::http::header::X_FRAME_OPTIONS,
        axum::http::HeaderValue::from_static("DENY"),
    );
    headers.insert(
        axum::http::header::REFERRER_POLICY,
        axum::http::HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        axum::http::HeaderName::from_static("permissions-policy"),
        axum::http::HeaderValue::from_static(
            "camera=(), microphone=(), geolocation=(), payment=()",
        ),
    );
    headers.insert(
        axum::http::HeaderName::from_static("x-xss-protection"),
        axum::http::HeaderValue::from_static("0"),
    );
    res
}

fn cors_layer() -> CorsLayer {
    let origins: Vec<axum::http::HeaderValue> = crate::app::app_origins()
        .into_iter()
        .filter_map(|o| o.parse().ok())
        .collect();

    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderName::from_static("x-spotify-id"),
            axum::http::HeaderName::from_static("x-user-id"),
            axum::http::HeaderName::from_static("x-user-signature"),
            axum::http::HeaderName::from_static("x-admin-token"),
            axum::http::HeaderName::from_static("x-app-id"),
            axum::http::HeaderName::from_static("x-client-id"),
        ])
        .expose_headers([
            axum::http::HeaderName::from_static("x-ratelimit-limit"),
            axum::http::HeaderName::from_static("x-ratelimit-remaining"),
            axum::http::HeaderName::from_static("retry-after"),
        ])
}

fn apply_layers(app: axum::Router, state: AppState, timeout_secs: u64) -> axum::Router {
    app.layer(middleware::from_fn_with_state(state, rate_limit_middleware))
        .layer(cors_layer())
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(timeout_secs),
        ))
        .layer(DefaultBodyLimit::max(
            crate::constants::DEFAULT_BODY_LIMIT_BYTES,
        ))
        .layer(middleware::from_fn(security_headers_middleware))
}
