use crate::state::{AppState, RateLimitEntry};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Returns `true` if the request is allowed, `false` if rate-limited.
/// Uses a sliding window algorithm based on millisecond timestamps.
pub fn check_rate_limit(state: &Arc<AppState>, client_ip: &str) -> bool {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let window_ms = state.rate_limit_window_ms;
    let max_requests = state.rate_limit_max_requests;
    let cutoff = now_ms.saturating_sub(window_ms);

    let mut entry = state
        .rate_limits
        .entry(client_ip.to_string())
        .or_insert_with(|| RateLimitEntry {
            timestamps: Vec::new(),
        });

    // Remove timestamps outside the sliding window
    entry.timestamps.retain(|&ts| ts > cutoff);

    // Check if the request is within the limit
    if entry.timestamps.len() as u64 >= max_requests {
        return false;
    }

    // Record the current request timestamp
    entry.timestamps.push(now_ms);
    true
}
