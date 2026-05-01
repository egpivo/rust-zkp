---
layout: ../../layouts/Layout.astro
title: "Backend Basics: HTTP API for ZKP Rollup"
---

# Backend Basics: HTTP API for ZKP Rollup

Building an HTTP server in Rust that exposes the rollup state via REST endpoints.

## Why Add a Server Layer

The ZKP library is just code. To make it usable:
- **Other services** need to submit transactions (HTTP)
- **Users / dashboards** want to query state (HTTP)
- **Provers / verifiers** in real systems are network services

A server turns your library into a runnable service that anyone with the address can interact with.

## Tech Stack Choices

| Concern | Choice | Why |
|---------|--------|-----|
| HTTP framework | `axum` | Type-safe, idiomatic, by tokio team |
| Async runtime | `tokio` | Industry standard for Rust async |
| JSON serialization | `serde` + `serde_json` | Standard for typed JSON in Rust |
| Shared state | `Arc<Mutex<T>>` | Multi-handler safety |

Alternatives:
- `actix-web` — older, faster microbenchmarks, steeper learning curve
- `rocket` — pretty syntax but less ecosystem alignment
- For our purposes: **axum is the right default**

## Async in Rust

### `async fn`

```rust
async fn health() -> &'static str { "ok" }
```

`async` makes a function return a `Future` instead of executing immediately.

```rust
async fn foo() -> i32 { 42 }

// Equivalent to:
fn foo() -> impl Future<Output = i32> {
    async { 42 }
}
```

A future doesn't run until **someone awaits it**.

### `#[tokio::main]`

```rust
#[tokio::main]
async fn main() { ... }
```

This macro expands to:
```rust
fn main() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        // your code
    });
}
```

Rust has no built-in async executor. You pick one (`tokio` is the default).

### Why async helps servers

When the server is waiting for a request:
- **Sync**: thread is blocked, doing nothing
- **Async**: thread can serve other connections in the meantime

That's why one async thread can handle thousands of connections — most of the time it's just idling waiting on I/O.

## Anatomy of an axum Server

```rust
let app = Router::new()
    .route("/", get(|| async { "rollup api" }))
    .route("/health", get(health))
    .with_state(shared);

let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
axum::serve(listener, app).await.unwrap();
```

### Step by step
- `Router::new()` — empty router
- `.route(path, get(handler))` — bind GET on `path` to `handler`
- `.with_state(shared)` — inject shared data into all handlers
- `TcpListener::bind("0.0.0.0:3000")` — bind to port 3000 on all interfaces
- `axum::serve(listener, app)` — run the server forever

### `0.0.0.0` vs `localhost`

| Address | Meaning | Used by |
|---------|---------|---------|
| `0.0.0.0` | "Listen on all network interfaces" | Server when binding |
| `127.0.0.1` / `localhost` | "This machine" | Client when connecting |

Browsers won't accept `0.0.0.0` as a URL — connect to `localhost:3000` instead.

## Shared State: `Arc<Mutex<T>>`

Handlers run on different tasks (potentially different threads). They all need to access the same `RollupState`. This is the canonical Rust pattern:

```rust
type SharedState = Arc<Mutex<RollupState>>;

let shared = Arc::new(Mutex::new(rollup_state));

let app = Router::new()
    .route(...)
    .with_state(shared);
```

### What each layer does
- **`Arc<T>`** (Atomic Reference Counting): allows multiple owners to share the same data; thread-safe reference counting
- **`Mutex<T>`**: only one owner can mutate at a time; `lock()` blocks until access is granted
- Together: any number of handlers can hold a reference (`Arc`), but they take turns writing (`Mutex`)

### Inside a handler

```rust
async fn get_balance(
    Path(id): Path<u32>,
    State(state): State<SharedState>,
) -> String {
    let s = state.lock().await;
    s.accounts.get(&id).map(|a| a.balance.to_string()).unwrap_or("not found".into())
}
```

- `state.lock().await` — acquire the lock asynchronously (won't block the runtime, just this task)
- The lock is automatically released when `s` goes out of scope (RAII)

### Why not just `Mutex<T>` (without Arc)?

A `Mutex<T>` lives somewhere — one place. Multiple handlers each need a reference. With `Arc`, they each clone an `Arc` (cheap pointer + atomic counter increment) and all point to the same `Mutex`.

### Why `tokio::sync::Mutex` not `std::sync::Mutex`?

- `std::sync::Mutex::lock()` — blocking, will halt the entire thread
- `tokio::sync::Mutex::lock().await` — async, only suspends this task; thread can do other work

For an async server, always use `tokio::sync::Mutex`.

## Axum Extractors

Axum handlers can pull data from the request via "extractors":

| Extractor | Source |
|-----------|--------|
| `Path<T>` | URL path params (e.g., `/balance/:id`) |
| `Query<T>` | URL query string (`?key=value`) |
| `Json<T>` | Request body, parsed as JSON |
| `State<T>` | Shared application state |
| `Headers` | HTTP headers |

The order doesn't matter — axum looks at the type signature and figures out where each value comes from.

```rust
async fn handler(
    Path(id): Path<u32>,           // from URL: /balance/123
    State(state): State<MyState>,  // from with_state(...)
    Json(body): Json<MyRequest>,   // from request body
) -> Result<Json<MyResponse>, StatusCode> { ... }
```

## Lifetime: `&'static`

Saw `&'static str` in the health handler:

```rust
async fn health() -> &'static str { "ok" }
```

- `&` — borrow / reference
- `'static` — lifetime that lasts the entire program
- `str` — string slice type

String literals like `"ok"` are stored in the binary's read-only data segment, so they never get freed — their references are `'static`.

Lifetimes ensure references don't outlive the data they point to. The compiler enforces this at compile time.

## Project Structure

```
src/
  main.rs       — server entry point (binary)
  lib.rs        — ZKP library (public modules)
  state.rs      — RollupState (used by both lib and main)
  ...
```

Cargo automatically links them. `main.rs` can `use zkp::state::State;` because `lib.rs` exposes it.

For multiple binaries: put them in `src/bin/server.rs`, `src/bin/cli.rs`. Each gets its own `cargo run --bin name`.

## Manual Testing with curl

```bash
curl http://localhost:3000/health
# -> ok

curl http://localhost:3000/state-root
# -> some big number

curl http://localhost:3000/balance/1
# -> 100

curl http://localhost:3000/balance/999
# -> account not found
```

For JSON later:
```bash
curl -X POST http://localhost:3000/tx \
  -H 'Content-Type: application/json' \
  -d '{"from": 1, "to": 2, "amount": 30}'
```

## What's Next

After this read-only API:
1. **POST /tx** — accept signed transactions
2. **Persistence** — `sled` or `rocksdb` to survive restarts
3. **Mempool + batch builder** — tokio channels, background tasks
4. **Tracing** — structured logging instead of `println!`
5. **Integration tests** — `reqwest` to hit the actual server

Each of these is a useful Rust pattern that recurs in any real backend.
