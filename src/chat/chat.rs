use axum::{
    extract::{ConnectInfo, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

use crate::state::{AppState, CacheEntry};
use crate::{embedding, rate_limit, security};

#[derive(Deserialize)]
pub struct ChatInput {
    pub prompt: String,
}

#[derive(Serialize)]
pub struct ChatOutput {
    pub response: String,
    pub cached: bool,
    pub request_id: String,
}

#[derive(Serialize)]
pub struct ErrorOutput {
    pub error: String,
    pub code: String,
}

pub async fn chat(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<ChatInput>,
) -> Response {
    let start = Instant::now();
    let request_id = uuid::Uuid::new_v4().to_string();
    let client_ip = addr.ip().to_string();

    // 1. Increment total requests
    state.metrics.total_requests.fetch_add(1, Ordering::Relaxed);

    info!(request_id = %request_id, client_ip = %client_ip, "Incoming chat request");

    // 2. Rate limit check
    if !rate_limit::check_rate_limit(&state, &client_ip) {
        state.metrics.rate_limited.fetch_add(1, Ordering::Relaxed);
        warn!(request_id = %request_id, client_ip = %client_ip, "Rate limit exceeded");

        let body = Json(ErrorOutput {
            error: "Rate limit exceeded. Please slow down.".to_string(),
            code: "RATE_LIMITED".to_string(),
        });
        return (StatusCode::TOO_MANY_REQUESTS, body).into_response();
    }

    // 3. Security check
    let security_result = security::check_prompt(&payload.prompt);
    if security_result.blocked {
        state.metrics.blocked_security.fetch_add(1, Ordering::Relaxed);
        warn!(
            request_id = %request_id,
            reason = %security_result.reason,
            "Security check blocked request"
        );

        let body = Json(ErrorOutput {
            error: security_result.reason,
            code: "SECURITY_BLOCKED".to_string(),
        });
        return (StatusCode::FORBIDDEN, body).into_response();
    }

    // 4. Semantic cache lookup
    let query_embedding = embedding::embed(&payload.prompt);
    let mut best_similarity = 0.0f64;
    let mut best_response: Option<String> = None;
    let mut expired_keys: Vec<String> = Vec::new();

    for entry in state.cache.iter() {
        if entry.value().is_expired() {
            expired_keys.push(entry.key().clone());
            continue;
        }

        let sim = embedding::cosine_similarity(&query_embedding, &entry.value().embedding);
        if sim > best_similarity {
            best_similarity = sim;
            if sim >= state.cache_similarity_threshold {
                best_response = Some(entry.value().response.clone());
            }
        }
    }

    // Lazily remove expired entries found during lookup
    for key in expired_keys {
        state.cache.remove(&key);
    }

    // 5. Cache hit
    if let Some(cached_response) = best_response {
        state.metrics.cache_hits.fetch_add(1, Ordering::Relaxed);

        let latency = start.elapsed().as_millis() as u64;
        state.metrics.total_latency_ms.fetch_add(latency, Ordering::Relaxed);
        state.metrics.latency_count.fetch_add(1, Ordering::Relaxed);

        info!(
            request_id = %request_id,
            similarity = %best_similarity,
            latency_ms = %latency,
            "Cache hit"
        );

        return (
            StatusCode::OK,
            Json(ChatOutput {
                response: cached_response,
                cached: true,
                request_id,
            }),
        )
            .into_response();
    }

    // 6. Cache miss — forward to upstream
    state.metrics.cache_misses.fetch_add(1, Ordering::Relaxed);

    let upstream_url = format!("{}/chat", state.upstream_url);
    let upstream_body = serde_json::json!({ "prompt": payload.prompt });

    let upstream_result = state
        .http_client
        .post(&upstream_url)
        .json(&upstream_body)
        .send()
        .await;

    match upstream_result {
        Err(e) => {
            state.metrics.upstream_errors.fetch_add(1, Ordering::Relaxed);
            warn!(request_id = %request_id, error = %e, "Upstream request failed");

            let latency = start.elapsed().as_millis() as u64;
            state.metrics.total_latency_ms.fetch_add(latency, Ordering::Relaxed);
            state.metrics.latency_count.fetch_add(1, Ordering::Relaxed);

            let body = Json(ErrorOutput {
                error: format!("Upstream error: {}", e),
                code: "UPSTREAM_ERROR".to_string(),
            });
            (StatusCode::BAD_GATEWAY, body).into_response()
        }
        Ok(resp) => {
            let status = resp.status();

            if !status.is_success() {
                state.metrics.upstream_errors.fetch_add(1, Ordering::Relaxed);
                warn!(
                    request_id = %request_id,
                    upstream_status = %status,
                    "Upstream returned non-success status"
                );

                let latency = start.elapsed().as_millis() as u64;
                state.metrics.total_latency_ms.fetch_add(latency, Ordering::Relaxed);
                state.metrics.latency_count.fetch_add(1, Ordering::Relaxed);

                let body = Json(ErrorOutput {
                    error: format!("Upstream returned status {}", status),
                    code: "UPSTREAM_ERROR".to_string(),
                });
                return (StatusCode::BAD_GATEWAY, body).into_response();
            }

            // Parse upstream response
            let upstream_text = match resp.text().await {
                Ok(t) => t,
                Err(e) => {
                    state.metrics.upstream_errors.fetch_add(1, Ordering::Relaxed);
                    let body = Json(ErrorOutput {
                        error: format!("Failed to read upstream response: {}", e),
                        code: "UPSTREAM_ERROR".to_string(),
                    });
                    return (StatusCode::BAD_GATEWAY, body).into_response();
                }
            };

            // Extract the response string from upstream JSON (try "response" field, else raw text)
            let response_text = serde_json::from_str::<serde_json::Value>(&upstream_text)
                .ok()
                .and_then(|v| v.get("response").and_then(|r| r.as_str()).map(|s| s.to_string()))
                .unwrap_or(upstream_text);

            // 7. Store in cache
            let cache_key = uuid::Uuid::new_v4().to_string();
            let now_secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            state.cache.insert(
                cache_key,
                CacheEntry {
                    prompt: payload.prompt,
                    response: response_text.clone(),
                    embedding: query_embedding,
                    created_at: now_secs,
                    ttl_secs: state.cache_ttl_secs,
                },
            );

            // 8. Record latency
            let latency = start.elapsed().as_millis() as u64;
            state.metrics.total_latency_ms.fetch_add(latency, Ordering::Relaxed);
            state.metrics.latency_count.fetch_add(1, Ordering::Relaxed);

            info!(
                request_id = %request_id,
                latency_ms = %latency,
                "Upstream response cached and returned"
            );

            // 9. Return response
            (
                StatusCode::OK,
                Json(ChatOutput {
                    response: response_text,
                    cached: false,
                    request_id,
                }),
            )
                .into_response()
        }
    }
}
