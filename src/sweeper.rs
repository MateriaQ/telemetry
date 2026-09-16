use crate::app::all_apps;
use crate::constants::{LIVE_WINDOW_SECS, SWEEP_INTERVAL_SECS, live_key};
use crate::utils::current_unix_timestamp;
use redis::aio::ConnectionManager;
use std::time::Duration;

pub fn start_sweeper(mut conn: ConnectionManager) {
    tokio::spawn(async move {
        tracing::info!(
            "Multi-app live user sweeper started (interval: {}s, live window: {}s)",
            SWEEP_INTERVAL_SECS,
            LIVE_WINDOW_SECS
        );

        let mut interval = tokio::time::interval(Duration::from_secs(SWEEP_INTERVAL_SECS));

        loop {
            interval.tick().await;
            let now = current_unix_timestamp();
            let cutoff = now - LIVE_WINDOW_SECS;

            let mut pipe = redis::pipe();
            pipe.atomic();

            for app in all_apps() {
                pipe.zrembyscore(live_key(app.id), "-inf", cutoff);
            }
            // Clean legacy single-app key if present
            pipe.zrembyscore("app:users:live", "-inf", cutoff);

            let result: redis::RedisResult<Vec<i64>> = pipe.query_async(&mut conn).await;

            match result {
                Ok(removed_counts) => {
                    let total_removed: i64 = removed_counts.iter().sum();
                    if total_removed > 0 {
                        tracing::info!(
                            "Swept {total_removed} stale live user(s) across all applications"
                        );
                    }
                }
                Err(e) => {
                    tracing::error!("Redis error during multi-app background sweep: {e}");
                }
            }
        }
    });
}
