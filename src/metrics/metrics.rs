use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::state::AppState;

pub async fn metrics(State(state): State<Arc<AppState>>) -> Response {
    let total_requests = state.metrics.total_requests.load(Ordering::Relaxed);
    let cache_hits = state.metrics.cache_hits.load(Ordering::Relaxed);
    let cache_misses = state.metrics.cache_misses.load(Ordering::Relaxed);
    let rate_limited = state.metrics.rate_limited.load(Ordering::Relaxed);
    let blocked_security = state.metrics.blocked_security.load(Ordering::Relaxed);
    let upstream_errors = state.metrics.upstream_errors.load(Ordering::Relaxed);
    let avg_latency_ms = state.metrics.avg_latency_ms();

    let body = format!(
        "# HELP gatecache_total_requests Total number of requests received\n\
         # TYPE gatecache_total_requests counter\n\
         gatecache_total_requests {total_requests}\n\
         \n\
         # HELP gatecache_cache_hits Total number of semantic cache hits\n\
         # TYPE gatecache_cache_hits counter\n\
         gatecache_cache_hits {cache_hits}\n\
         \n\
         # HELP gatecache_cache_misses Total number of semantic cache misses\n\
         # TYPE gatecache_cache_misses counter\n\
         gatecache_cache_misses {cache_misses}\n\
         \n\
         # HELP gatecache_rate_limited_requests Total number of rate-limited requests\n\
         # TYPE gatecache_rate_limited_requests counter\n\
         gatecache_rate_limited_requests {rate_limited}\n\
         \n\
         # HELP gatecache_blocked_requests Total number of requests blocked by security\n\
         # TYPE gatecache_blocked_requests counter\n\
         gatecache_blocked_requests {blocked_security}\n\
         \n\
         # HELP gatecache_upstream_errors Total number of upstream errors\n\
         # TYPE gatecache_upstream_errors counter\n\
         gatecache_upstream_errors {upstream_errors}\n\
         \n\
         # HELP gatecache_avg_latency_ms Average request latency in milliseconds\n\
         # TYPE gatecache_avg_latency_ms gauge\n\
         gatecache_avg_latency_ms {avg_latency_ms:.3}\n",
    );

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        body,
    )
        .into_response()
}
