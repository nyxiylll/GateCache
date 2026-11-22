# 🛡️ Gatecache

A lightweight, high-performance reverse-proxy middleware written in **Rust** that sits in front of any LLM or AI server. Drop it once into your stack and get semantic caching, rate limiting, and prompt security for free — no changes needed to your upstream server.

```
Client ──► Gatecache :3000 ──► Upstream LLM / AI Server
                │
                ├── ① Rate Limiting      (sliding window per IP)
                ├── ② Security Checks    (regex-based prompt injection & PII detection)
                ├── ③ Semantic Cache     (n-gram embeddings + cosine similarity)
                │       ├── HIT  → instant cached response
                │       └── MISS → proxy to upstream, cache result
                └── ④ Metrics           (/metrics — Prometheus format)
```

---

## ✨ Features

| Feature | Details |
|---|---|
| **Semantic Cache** | FNV-1a character n-gram embeddings (512-dim) with cosine similarity. Catches rephrased duplicates — not just exact matches. Configurable similarity threshold and TTL. |
| **Rate Limiting** | Sliding window per client IP using lock-free `DashMap`. Fully configurable requests/window. |
| **Prompt Security** | 15 compiled regex patterns across 4 categories — blocks prompt injection, jailbreaks, PII (SSN, credit cards, phone numbers), SQL injection, shell injection, and null bytes. |
| **Reverse Proxy** | Forwards cache misses to any upstream via `reqwest`. Works with any server that accepts `{"prompt": "..."}` JSON. |
| **Prometheus Metrics** | `/metrics` endpoint with counters for requests, cache hits/misses, rate-limited, blocked, upstream errors, and avg latency. |
| **Zero external deps** | No vector DB, no external embedding API, no Redis required. Pure in-memory, self-contained. |
| **Docker ready** | Multi-stage Dockerfile + `docker-compose.yml` included. |

---

## 🚀 Quick Start

### Run locally

```bash
# Clone the repo
git clone https://github.com/your-username/gatecache.git
cd gatecache

# Set your upstream URL and run
UPSTREAM_URL=http://localhost:8080 RUST_LOG=info cargo run
```

Gatecache will listen on **port 3000**.

### Run with Docker

```bash
# Edit UPSTREAM_URL in docker-compose.yml, then:
docker compose up --build
```

---

## 🔌 API

### `POST /chat`

Send a prompt. Gatecache will check the cache, run security checks, apply rate limiting, and proxy to upstream on a miss.

**Request**
```json
{
  "prompt": "What is the capital of France?"
}
```

**Response (cache miss — proxied to upstream)**
```json
{
  "response": "The capital of France is Paris.",
  "cached": false,
  "request_id": "f47ac10b-58cc-4372-a567-0e02b2c3d479"
}
```

**Response (cache hit — returned instantly)**
```json
{
  "response": "The capital of France is Paris.",
  "cached": true,
  "request_id": "b2c3d479-..."
}
```

**Error responses**

| Status | Code | Reason |
|---|---|---|
| `429` | `RATE_LIMITED` | Too many requests from this IP |
| `403` | `SECURITY_BLOCKED` | Prompt failed security checks |
| `502` | `UPSTREAM_ERROR` | Upstream server unreachable or errored |

---

### `GET /health_check`

```
200 OK — "Healthy"
```

---

### `GET /metrics`

Returns Prometheus text format:

```
# HELP gatecache_total_requests Total number of requests received
# TYPE gatecache_total_requests counter
gatecache_total_requests 142

# HELP gatecache_cache_hits Total number of semantic cache hits
# TYPE gatecache_cache_hits counter
gatecache_cache_hits 97

# HELP gatecache_cache_misses Total number of semantic cache misses
# TYPE gatecache_cache_misses counter
gatecache_cache_misses 45

# HELP gatecache_rate_limited_requests Total number of rate-limited requests
# TYPE gatecache_rate_limited_requests counter
gatecache_rate_limited_requests 3

# HELP gatecache_blocked_requests Total number of requests blocked by security
# TYPE gatecache_blocked_requests counter
gatecache_blocked_requests 2

# HELP gatecache_upstream_errors Total number of upstream errors
# TYPE gatecache_upstream_errors counter
gatecache_upstream_errors 0

# HELP gatecache_avg_latency_ms Average request latency in milliseconds
# TYPE gatecache_avg_latency_ms gauge
gatecache_avg_latency_ms 1.243
```

---

## ⚙️ Configuration

All configuration is via environment variables. No config file needed.

| Variable | Default | Description |
|---|---|---|
| `UPSTREAM_URL` | `http://localhost:8080` | Base URL of your upstream LLM/AI server |
| `CACHE_SIMILARITY_THRESHOLD` | `0.85` | Cosine similarity threshold for a cache hit (0.0–1.0). Lower = more aggressive caching. |
| `CACHE_TTL_SECS` | `3600` | How long (seconds) a cache entry lives before expiry |
| `RATE_LIMIT_MAX_REQUESTS` | `60` | Max requests per IP per window |
| `RATE_LIMIT_WINDOW_MS` | `60000` | Sliding window size in milliseconds (default: 1 minute) |
| `MAX_PROMPT_LENGTH` | `10000` | Maximum allowed prompt length in characters |
| `RUST_LOG` | `info` | Log level (`trace`, `debug`, `info`, `warn`, `error`) |

---

## 🛡️ Security Checks

Gatecache inspects every prompt before forwarding. Blocked requests never reach your upstream.

### Prompt Injection
- `ignore (all/previous) instructions`
- `forget everything / what you were told`
- `you are now <X>` / `act as <X>` / `pretend you are`
- `disregard / override your instructions`
- `system:` / `[system]` / `<system>` prefix injection
- `{{template}}` injection
- `<|special_token|>` injection

### Jailbreak Attempts
- Known keywords: `jailbreak`, `DAN mode`, `do anything now`, `developer mode`
- `bypass the safety / filter / restriction`
- Fictional/hypothetical world framing

### PII Detection
- **Social Security Numbers** — `123-45-6789`
- **Credit card numbers** — 16-digit patterns
- **Phone numbers** — various formats

### Code Injection
- SQL injection patterns (`SELECT...FROM`, `DROP TABLE`, etc.)
- Shell metacharacter sequences (`` ; && | ` $() ``), 
- Null bytes (`\x00`)

---

## 🧠 How the Semantic Cache Works

Instead of exact-match string caching, Gatecache uses a **character n-gram hashing** approach:

1. **Tokenize** — lowercase the prompt, strip punctuation, split into words
2. **N-grams** — generate character bigrams and trigrams from each word
3. **Hash** — FNV-1a hash each n-gram into a 512-dimensional bucket vector
4. **Normalize** — L2-normalize to a unit vector
5. **Compare** — cosine similarity against all cached embeddings

This means `"What is the capital of France?"` and `"France's capital city?"` will both hit the same cache entry as long as similarity ≥ threshold (default `0.85`).

Expired entries are cleaned up **lazily** during lookups, plus a **background task** sweeps every 5 minutes.

---

## 🏗️ Architecture

```
src/
├── main.rs              # Entry point, tracing init, TCP listener
├── lib.rs               # Router setup, AppState injection
├── state.rs             # AppState, Metrics (AtomicU64), CacheEntry, RateLimitEntry
├── embedding.rs         # FNV-1a n-gram hashing, cosine similarity
├── security.rs          # 15 lazy Regex patterns, check_prompt()
├── rate_limit.rs        # Sliding window rate limiter (DashMap)
├── chat/
│   ├── mod.rs
│   └── chat.rs          # Main pipeline handler
├── metrics/
│   ├── mod.rs
│   └── metrics.rs       # Prometheus text format output
└── health_check/
    ├── mod.rs
    └── heath_check.rs   # Simple health check
```

---

## 🐳 Docker

### Dockerfile (multi-stage)

The image uses a two-stage build: a `rust:1.82-slim` builder stage compiles the binary, then copies only the binary into a minimal `debian:bookworm-slim` runtime image.

```bash
docker build -t gatecache .
docker run -p 3000:3000 -e UPSTREAM_URL=http://your-server:8080 gatecache
```

### docker-compose.yml

```yaml
services:
  gatecache:
    build: .
    ports:
      - "3000:3000"
    environment:
      - UPSTREAM_URL=http://your-llm-server:8080
      - CACHE_SIMILARITY_THRESHOLD=0.85
      - CACHE_TTL_SECS=3600
      - RATE_LIMIT_MAX_REQUESTS=60
      - RATE_LIMIT_WINDOW_MS=60000
      - MAX_PROMPT_LENGTH=10000
      - RUST_LOG=info
    restart: unless-stopped
```

---

## 🔧 Development

```bash
# Build
cargo build

# Run tests
cargo test

# Build release binary
cargo build --release

# Run with debug logging
RUST_LOG=debug cargo run
```

---

## 📦 Dependencies

| Crate | Purpose |
|---|---|
| `axum` | HTTP server framework |
| `tokio` | Async runtime |
| `reqwest` | HTTP client for upstream proxying |
| `regex` + `once_cell` | Compiled security patterns |
| `dashmap` | Lock-free concurrent hashmaps |
| `serde` / `serde_json` | JSON serialization |
| `uuid` | Unique request and cache IDs |
| `tracing` / `tracing-subscriber` | Structured logging |
| `tower-http` | HTTP middleware utilities |

---

## 📄 License

MIT
