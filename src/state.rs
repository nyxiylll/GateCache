use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use dashmap::DashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use reqwest::Client;

pub struct Metrics {
    pub total_requests: AtomicU64,
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
    pub rate_limited: AtomicU64,
    pub blocked_security: AtomicU64,
    pub upstream_errors: AtomicU64,
    pub total_latency_ms: AtomicU64,
    pub latency_count: AtomicU64,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            rate_limited: AtomicU64::new(0),
            blocked_security: AtomicU64::new(0),
            upstream_errors: AtomicU64::new(0),
            total_latency_ms: AtomicU64::new(0),
            latency_count: AtomicU64::new(0),
        }
    }

    pub fn avg_latency_ms(&self) -> f64 {
        let count = self.latency_count.load(Ordering::Relaxed);
        if count == 0 {
            return 0.0;
        }
        self.total_latency_ms.load(Ordering::Relaxed) as f64 / count as f64
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

pub struct CacheEntry {
    pub prompt: String,
    pub response: String,
    pub embedding: Vec<f64>,
    pub created_at: u64,
    pub ttl_secs: u64,
}

impl CacheEntry {
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now > self.created_at + self.ttl_secs
    }
}

pub struct RateLimitEntry {
    pub timestamps: Vec<u64>, // sliding window of request timestamps in ms
}

pub struct AppState {
    pub metrics: Arc<Metrics>,
    pub cache: Arc<DashMap<String, CacheEntry>>,
    pub rate_limits: Arc<DashMap<String, RateLimitEntry>>,
    pub upstream_url: String,
    pub cache_similarity_threshold: f64,
    pub cache_ttl_secs: u64,
    pub rate_limit_max_requests: u64,
    pub rate_limit_window_ms: u64,
    pub max_prompt_length: usize,
    pub http_client: Client,
}

impl AppState {
    pub fn new() -> Self {
        let upstream_url = std::env::var("UPSTREAM_URL")
            .unwrap_or_else(|_| "http://localhost:8080".to_string());

        let cache_similarity_threshold = std::env::var("CACHE_SIMILARITY_THRESHOLD")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.85);

        let cache_ttl_secs = std::env::var("CACHE_TTL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(3600);

        let rate_limit_max_requests = std::env::var("RATE_LIMIT_MAX_REQUESTS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(60);

        let rate_limit_window_ms = std::env::var("RATE_LIMIT_WINDOW_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(60000);

        let max_prompt_length = std::env::var("MAX_PROMPT_LENGTH")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(10000);

        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");

        let state = Self {
            metrics: Arc::new(Metrics::new()),
            cache: Arc::new(DashMap::new()),
            rate_limits: Arc::new(DashMap::new()),
            upstream_url,
            cache_similarity_threshold,
            cache_ttl_secs,
            rate_limit_max_requests,
            rate_limit_window_ms,
            max_prompt_length,
            http_client,
        };

        // Spawn background cache cleanup task
        let cache_clone = state.cache.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                cache_clone.retain(|_, v| !v.is_expired());
                tracing::debug!("Cache cleanup complete, {} entries remaining", cache_clone.len());
            }
        });

        state
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
