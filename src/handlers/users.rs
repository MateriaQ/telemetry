use crate::app::{all_apps, find_app, tracks_lifetime};
use crate::constants::{LIVE_WINDOW_SECS, daily_key, lifetime_key, live_key};
use crate::models::{
    ApiError, AppTelemetry, DailyStat, LiveUsersResponse, TelemetryOverview, UsersResponse,
};
use crate::state::AppState;
use crate::utils::{current_unix_timestamp, get_last_7_days};
use axum::extract::{RawQuery, State};
use axum::http::{StatusCode, header};
use rovo::response::Json;
use rovo::rovo;

const CACHE_HEADERS: [(header::HeaderName, &str); 3] = [
    (
        header::CACHE_CONTROL,
        "public, max-age=1, s-maxage=2, stale-while-revalidate=2",
    ),
    (
        header::HeaderName::from_static("cdn-cache-control"),
        "max-age=2, stale-while-revalidate=2",
    ),
    (
        header::HeaderName::from_static("cloudflare-cdn-cache-control"),
        "max-age=2, stale-while-revalidate=2",
    ),
];

fn api_error(status: StatusCode, message: impl Into<String>) -> (StatusCode, Json<ApiError>) {
    (
        status,
        Json(ApiError {
            error: message.into(),
        }),
    )
}

pub(crate) async fn fetch_single_app_telemetry(
    app: &'static crate::app::AppConfig,
    redis: &mut redis::aio::ConnectionManager,
) -> Result<AppTelemetry, StatusCode> {
    let now = current_unix_timestamp();
    let cutoff = now - LIVE_WINDOW_SECS;
    let dates = get_last_7_days(now);

    let mut pipe = redis::pipe();
    let app_str = app.id;
    pipe.cmd("ZCOUNT")
        .arg(live_key(app_str))
        .arg(cutoff)
        .arg("+inf");

    if tracks_lifetime(app_str) {
        pipe.cmd("SCARD").arg(lifetime_key(app_str));
    }
    for date in &dates {
        pipe.cmd("SCARD").arg(daily_key(app_str, date));
    }

    let raw_results: Vec<u64> = pipe.query_async(redis).await.map_err(|e| {
        tracing::error!(
            "Redis connection error fetching app '{}' telemetry: {e}",
            app.id
        );
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    let mut cursor = 0;
    let live = raw_results.get(cursor).copied().unwrap_or(0);
    cursor += 1;

    let total = if tracks_lifetime(app.id) {
        let t = raw_results.get(cursor).copied().unwrap_or(0);
        cursor += 1;
        Some(t)
    } else {
        None
    };

    let mut history = Vec::with_capacity(dates.len());
    let mut weekly_max = 0u64;

    for date in &dates {
        let count = raw_results.get(cursor).copied().unwrap_or(0);
        cursor += 1;
        if count > weekly_max {
            weekly_max = count;
        }
        history.push(DailyStat {
            date: date.clone(),
            count,
        });
    }

    Ok(AppTelemetry {
        id: app.id.to_string(),
        name: app.name.to_string(),
        website: app.website.map(ToString::to_string),
        live,
        total,
        weekly_history: history,
        weekly_max,
    })
}

pub(crate) async fn fetch_total_live_count(
    redis: &mut redis::aio::ConnectionManager,
) -> Result<u64, StatusCode> {
    let now = current_unix_timestamp();
    let cutoff = now - LIVE_WINDOW_SECS;
    let mut pipe = redis::pipe();

    for app in all_apps() {
        pipe.cmd("ZCOUNT")
            .arg(live_key(app.id))
            .arg(cutoff)
            .arg("+inf");
    }

    let counts: Vec<u64> = pipe.query_async(redis).await.map_err(|e| {
        tracing::error!("Redis error fetching total live count: {e}");
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    Ok(counts.into_iter().sum())
}

pub(crate) async fn fetch_app_live_count(
    app_id: &str,
    redis: &mut redis::aio::ConnectionManager,
) -> Result<u64, StatusCode> {
    let now = current_unix_timestamp();
    let cutoff = now - LIVE_WINDOW_SECS;

    redis::cmd("ZCOUNT")
        .arg(live_key(app_id))
        .arg(cutoff)
        .arg("+inf")
        .query_async(redis)
        .await
        .map_err(|e| {
            tracing::error!("Redis error fetching live count for app '{app_id}': {e}");
            StatusCode::SERVICE_UNAVAILABLE
        })
}

fn extract_app_query(raw_query: Option<&str>) -> Option<&str> {
    raw_query.and_then(|q| {
        q.split('&').find_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            if parts.next()? == "app" {
                let val = parts.next()?;
                if val.is_empty() { None } else { Some(val) }
            } else {
                None
            }
        })
    })
}

/// Telemetry overview or single app stats
///
/// # Responses
///
/// 200: Json<UsersResponse> - Telemetry metrics
/// 404: Json<ApiError> - App not found
/// 503: Json<ApiError> - Redis unavailable
///
/// # Metadata
///
/// @tag telemetry
#[rovo]
pub async fn users(
    State(state): State<AppState>,
    RawQuery(raw_query): RawQuery,
) -> Result<
    ([(header::HeaderName, &'static str); 3], Json<UsersResponse>),
    (StatusCode, Json<ApiError>),
> {
    let mut redis = state.redis.clone();

    if let Some(app_query) = extract_app_query(raw_query.as_deref()) {
        let app = find_app(app_query)
            .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "application not found"))?;

        let app_telemetry = fetch_single_app_telemetry(app, &mut redis)
            .await
            .map_err(|status| api_error(status, "failed to read app metrics"))?;

        return Ok((CACHE_HEADERS, Json(UsersResponse::App(app_telemetry))));
    }

    let now = current_unix_timestamp();
    let cutoff = now - LIVE_WINDOW_SECS;
    let dates = get_last_7_days(now);

    let mut pipe = redis::pipe();

    for app in all_apps() {
        let app_str = app.id;
        pipe.cmd("ZCOUNT")
            .arg(live_key(app_str))
            .arg(cutoff)
            .arg("+inf");

        if tracks_lifetime(app_str) {
            pipe.cmd("SCARD").arg(lifetime_key(app_str));
        }
        for date in &dates {
            pipe.cmd("SCARD").arg(daily_key(app_str, date));
        }
    }

    let raw_results: Vec<u64> = pipe.query_async(&mut redis).await.map_err(|e| {
        tracing::error!("Redis connection error: failed to fetch multi-app telemetry: {e}");
        api_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "failed to read user metrics",
        )
    })?;

    let mut cursor = 0;
    let mut apps_telemetry = Vec::new();
    let mut aggregate_live = 0u64;
    let mut aggregate_total = 0u64;

    for app in all_apps() {
        let live = raw_results.get(cursor).copied().unwrap_or(0);
        cursor += 1;
        aggregate_live += live;

        let total = if tracks_lifetime(app.id) {
            let t = raw_results.get(cursor).copied().unwrap_or(0);
            cursor += 1;
            aggregate_total += t;
            Some(t)
        } else {
            None
        };

        let mut history = Vec::with_capacity(dates.len());
        let mut weekly_max = 0u64;

        for date in &dates {
            let count = raw_results.get(cursor).copied().unwrap_or(0);
            cursor += 1;
            if count > weekly_max {
                weekly_max = count;
            }
            history.push(DailyStat {
                date: date.clone(),
                count,
            });
        }

        apps_telemetry.push(AppTelemetry {
            id: app.id.to_string(),
            name: app.name.to_string(),
            website: app.website.map(ToString::to_string),
            live,
            total,
            weekly_history: history,
            weekly_max,
        });
    }

    Ok((
        CACHE_HEADERS,
        Json(UsersResponse::Overview(TelemetryOverview {
            live: aggregate_live,
            total: aggregate_total,
            apps: apps_telemetry,
        })),
    ))
}

/// Telemetry stats for a specific app
///
/// # Responses
///
/// 200: Json<AppTelemetry> - App telemetry stats
/// 404: Json<ApiError> - App not found
/// 503: Json<ApiError> - Redis unavailable
///
/// # Metadata
///
/// @tag telemetry
#[rovo]
pub async fn users_by_app(
    State(state): State<AppState>,
    axum::extract::Path(app_id): axum::extract::Path<String>,
) -> Result<
    ([(header::HeaderName, &'static str); 3], Json<AppTelemetry>),
    (StatusCode, Json<ApiError>),
> {
    let app = find_app(&app_id)
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "application not found"))?;

    let mut redis = state.redis.clone();
    let app_telemetry = fetch_single_app_telemetry(app, &mut redis)
        .await
        .map_err(|status| api_error(status, "failed to read app metrics"))?;

    Ok((CACHE_HEADERS, Json(app_telemetry)))
}

/// Live user count (overall or single app via ?app=)
///
/// # Responses
///
/// 200: Json<LiveUsersResponse> - Live user count
/// 404: Json<ApiError> - App not found
/// 503: Json<ApiError> - Redis unavailable
///
/// # Metadata
///
/// @tag telemetry
#[rovo]
pub async fn users_live(
    State(state): State<AppState>,
    RawQuery(raw_query): RawQuery,
) -> Result<
    (
        [(header::HeaderName, &'static str); 3],
        Json<LiveUsersResponse>,
    ),
    (StatusCode, Json<ApiError>),
> {
    let mut redis = state.redis.clone();

    if let Some(app_query) = extract_app_query(raw_query.as_deref()) {
        let app = find_app(app_query)
            .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "application not found"))?;

        let live = fetch_app_live_count(app.id, &mut redis)
            .await
            .map_err(|status| api_error(status, "failed to read live metrics"))?;

        return Ok((
            CACHE_HEADERS,
            Json(LiveUsersResponse {
                id: Some(app.id.to_string()),
                live,
            }),
        ));
    }

    let live = fetch_total_live_count(&mut redis)
        .await
        .map_err(|status| api_error(status, "failed to read live metrics"))?;

    Ok((CACHE_HEADERS, Json(LiveUsersResponse { id: None, live })))
}

/// Live user count for a specific app
///
/// # Responses
///
/// 200: Json<LiveUsersResponse> - Live user count for the app
/// 404: Json<ApiError> - App not found
/// 503: Json<ApiError> - Redis unavailable
///
/// # Metadata
///
/// @tag telemetry
#[rovo]
pub async fn app_live(
    State(state): State<AppState>,
    axum::extract::Path(app_id): axum::extract::Path<String>,
) -> Result<
    (
        [(header::HeaderName, &'static str); 3],
        Json<LiveUsersResponse>,
    ),
    (StatusCode, Json<ApiError>),
> {
    let app = find_app(&app_id)
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "application not found"))?;

    let mut redis = state.redis.clone();
    let live = fetch_app_live_count(app.id, &mut redis)
        .await
        .map_err(|status| api_error(status, "failed to read live metrics"))?;

    Ok((
        CACHE_HEADERS,
        Json(LiveUsersResponse {
            id: Some(app.id.to_string()),
            live,
        }),
    ))
}
