# Rate Limiting and API Key Auth

Two pieces of axum middleware that turn a learning toy into something you can leave running on the public internet.

## Why Both

| Concern | Without protection | With protection |
|---------|-------------------|-----------------|
| Account creation spam | Anyone can fill `accounts` | API key required |
| Request flooding | One client can saturate the server | 429 after burst |
| Resource exhaustion | OOM in mempool / disk | Bounded |
| Bad actors finding it | Just hits work | Slowed; visible in logs |

For our project: API key on `POST /accounts`, rate limit on everything.

## Tower Middleware Model

axum is built on `tower` — every middleware is a function that wraps a request handler.

```
       Request flow
       ───────────►
┌─────────────────────────────────┐
│ CorsLayer                       │
│  ┌────────────────────────────┐ │
│  │ GovernorLayer (rate limit) │ │
│  │  ┌─────────────────────┐   │ │
│  │  │ require_api_key     │   │ │
│  │  │  ┌───────────────┐  │   │ │
│  │  │  │ create_account│  │   │ │
│  │  │  └───────────────┘  │   │ │
│  │  └─────────────────────┘   │ │
│  └────────────────────────────┘ │
└─────────────────────────────────┘
```

A request enters from the outside (CORS) and unwinds to the handler in the middle. Each layer can:
- Inspect the request
- Reject (return early without calling `next.run`)
- Modify request before passing on
- Modify response after the handler returns

## Two Styles of Middleware

### 1. Function style (`middleware::from_fn`)

The simplest. A regular `async fn`:

```rust
async fn require_api_key(
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // ... check ...
    if ok {
        Ok(next.run(request).await)  // call the inner handler
    } else {
        Err(StatusCode::UNAUTHORIZED)  // bail out
    }
}

// Apply:
.layer(middleware::from_fn(require_api_key))
```

The function takes **extractors** (like `HeaderMap`) plus the special `Next`.

### 2. Tower service style

For complex stuff (state, configuration, lifetimes). You implement `tower::Service`. We don't use this here, but `GovernorLayer` is one example — it comes pre-built so you just configure and add it.

## API Key Auth Implementation

```rust
async fn require_api_key(
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let expected = std::env::var("API_KEY").unwrap_or_default();

    // No API_KEY env → skip auth (dev mode)
    if expected.is_empty() {
        return Ok(next.run(request).await);
    }

    let provided = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if provided == expected {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}
```

Key points:
- **Read env at request time**, not startup. Lets you change keys without restart.
- **Empty env → skip**. Dev mode without auth, prod with `API_KEY=...`.
- **Constant-time compare** would be more secure (`constant_time_eq` crate). For our learning toy, plain `==` is fine — but in production, timing attacks can leak the key character by character.

## Per-Route vs Global

```rust
// Global — applies to all routes
.layer(middleware::from_fn(require_api_key))

// Per-route — only this method on this path
.route("/accounts", post(create_account)
    .layer(middleware::from_fn(require_api_key)))

// Sub-router with shared middleware
let admin = Router::new()
    .route("/accounts", post(create_account))
    .route_layer(middleware::from_fn(require_api_key));
let app = Router::new().merge(admin)...;
```

For us: **only `POST /accounts` requires a key**. `POST /tx` doesn't, because the signature itself is the auth.

## Rate Limiting with `tower-governor`

Token bucket algorithm:

```
[●●●●●]  initial bucket (capacity = burst_size)
  │
  ▼ each request takes 1 token
[●●●●·]
[●●●··]
[●●···]
  ↻ +1 token added every (1 / per_second) seconds
```

If bucket is empty: 429 Too Many Requests.

### Configuration

```rust
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};

let conf = std::sync::Arc::new(
    GovernorConfigBuilder::default()
        .per_second(2)        // refill rate
        .burst_size(5)        // max bucket size
        .finish()
        .unwrap()
);

let governor_layer = GovernorLayer::new(conf);
```

Behavior:
- First 5 requests instant (drain the bucket)
- 6th request blocked (bucket empty)
- After ~3s: bucket refills to 5
- Steady state: max 2 req/sec sustained

### Connection Info Injection

`tower-governor` keys the bucket on **client IP** by default. To get the IP, axum needs to be told to expose it:

```rust
axum::serve(
    listener,
    app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
)
.await
.unwrap();
```

Without this, `tower-governor` returns 500 "Unable To Extract Key!".

`into_make_service_with_connect_info::<SocketAddr>()` injects the client `SocketAddr` into request extensions; governor pulls it from there.

### Alternative key extractors

```rust
use tower_governor::key_extractor::GlobalKeyExtractor;

GovernorConfigBuilder::default()
    .key_extractor(GlobalKeyExtractor)  // one bucket for everyone
    .finish()
```

Useful for:
- Dev/testing where IP detection is fiddly
- Behind a load balancer where every "client IP" looks the same (you'd want to extract from `X-Forwarded-For` instead)

For prod behind a reverse proxy, you typically:
1. Read `X-Forwarded-For` header
2. Use a custom `KeyExtractor`

## Choosing Limits

| Endpoint kind | Sensible default |
|---------------|------------------|
| Public read API | per_second 60, burst 100 |
| Public write API | per_second 10, burst 20 |
| Auth endpoint | per_second 1, burst 5 |
| Internal/admin | None (auth handles it) |

For our learning project: 2/s + burst 5 is too tight (the playground hits `/params` + `/tx` + `/accounts/X` in quick succession). For real demo: bump to ~20/s + burst 50.

## Layer Order Matters

```rust
let app = Router::new()
    .route(...)
    .layer(governor_layer)   // applied AFTER cors
    .layer(cors);            // applied FIRST (outermost)
```

Reading: `cors` is **outermost** — it sees the request first, handles `OPTIONS` preflight, and sets CORS headers on responses.

Why: if rate-limit blocked the OPTIONS preflight, browsers would never make the actual request, and you'd never see CORS errors. CORS first lets browsers correctly diagnose CORS problems.

General order:
1. **Outermost: connection-level concerns** — CORS, request ID, tracing
2. **Middle: rate limiting / quota**
3. **Inner: auth, parsing**
4. **Innermost: handler**

## Testing

```bash
# Auth (with API_KEY=secret123 set)
curl -i -X POST http://localhost:3000/accounts \
  -H "Content-Type: application/json" \
  -d '{"id":1,"balance":1,"pubkey":"1"}'
# → 401 Unauthorized

curl -i -X POST http://localhost:3000/accounts \
  -H "Content-Type: application/json" \
  -H "x-api-key: secret123" \
  -d '{"id":1,"balance":1,"pubkey":"1"}'
# → 200 / "account 1 created"

# Rate limit
for i in {1..10}; do
  curl -s -o /dev/null -w "$i: %{http_code} " http://localhost:3000/health
done
# 1: 200 2: 200 3: 200 4: 200 5: 200 6: 429 7: 429 ...
```

## Production Concerns

We've skipped:

| Concern | What you'd add |
|---------|----------------|
| Constant-time auth compare | `constant_time_eq` crate |
| Multiple API keys / rotation | DB lookup or HMAC-based tokens |
| OAuth / JWT | `jsonwebtoken` crate |
| `X-Forwarded-For` for real IP | Custom `KeyExtractor` |
| Per-endpoint limits | Multiple `route_layer` |
| Distributed rate limit (multi-instance) | Redis + `tower-redis-rate-limit` |
| Burst protection vs sustained protection | Two layered limiters |
| 429 with `Retry-After` header | Already done by `tower-governor` |
| Auth audit log | Inside the middleware fn, log success/fail |

These all fit naturally into the same `tower::Service` model — additive, not destructive. That's why people like axum/tower.

## Rust Skills Reinforced

- **`async fn` middleware** — `from_fn` to register a function as a layer
- **`Next`** — abstraction over "the rest of the request pipeline"
- **`HeaderMap` extractor** — type-safe header access
- **`Result<Response, StatusCode>`** — early-return as 4xx/5xx
- **`std::sync::Arc<T>`** — share the governor config across requests (it's read-only state)
- **Layer ordering** — onion model, last `.layer()` is innermost
- **Per-route vs global layering** — `.layer()` vs `.route_layer()`
- **`into_make_service_with_connect_info::<T>()`** — opt into peer info injection

## Mental Model

> Middleware = a function that wraps the handler.
> `Next.run(request)` = "call the inner thing, give me back its response."
> Layer order = onion: last `.layer(x)` is **outermost**.
> Per-route layer = `.route(path, handler.layer(x))`.

That covers 95% of real axum middleware code.
