# Backend: POST Endpoints, JSON, and Error Handling

Building on the read-only API, this covers writing endpoints that accept input and may fail.

## What Changed

We added two POST endpoints:

```rust
.route("/tx", post(submit_tx))
.route("/accounts", post(create_account))
```

These accept JSON request bodies and can return errors. Three new concepts:
1. **`Json<T>` extractor** — auto-deserialize request body
2. **`#[derive(Deserialize)]`** — let `serde` build a struct from JSON
3. **`Result<T, E>` as return type** — handle success vs error in one signature

## Serde: JSON ↔ Struct

`serde` is Rust's serialization framework. With derive macros, you get `JSON ↔ struct` conversion for free:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct CreateAccountRequest {
    id: u32,
    balance: u64,
    pubkey: BigUint,
}
```

- `Deserialize` — JSON → struct (incoming requests)
- `Serialize` — struct → JSON (outgoing responses)

You can derive both together: `#[derive(Deserialize, Serialize)]`.

### Field types

`serde` can convert standard types automatically:

| Rust type | JSON form |
|-----------|-----------|
| `i32, u32, i64, u64` | number |
| `String, &str` | string |
| `bool` | boolean |
| `Vec<T>` | array |
| `Option<T>` | value or null |
| Struct | object |
| Enum | depends on `#[serde(...)]` config |

### `BigUint` and serde

`num-bigint` doesn't implement `Serialize`/`Deserialize` by default. Enable it via Cargo feature:

```toml
num-bigint = { version = "0.4", features = ["serde"] }
```

The default format is **a `Vec<u32>` of internal limbs**, not a string:
```json
{ "r": [0], "z": [42, 1] }
```

This is ugly for humans but efficient for programs. To get pretty hex/decimal strings, you'd write a custom serde adapter (skip this for learning).

## The `Json<T>` Extractor

```rust
async fn submit_tx(
    State(state): State<SharedState>,
    Json(tx): Json<Transaction>,
) -> Result<String, (StatusCode, String)> {
    // tx is already a deserialized Transaction
}
```

What axum does behind the scenes:
1. Reads the request body
2. Validates `Content-Type: application/json`
3. Calls `serde_json::from_slice::<Transaction>(body)`
4. If parse fails, returns 400 with the error message — **without ever calling your handler**
5. If parse succeeds, your handler runs with the typed value

You don't have to write boilerplate parsing or error handling for malformed JSON. This is the power of axum's typed extractors.

## Returning Errors: `Result<T, E>`

Real handlers can fail. axum lets you return `Result<T, E>` where:
- `T` is the success body
- `E` is anything that implements `IntoResponse`

The simplest pattern is `(StatusCode, String)`:

```rust
async fn submit_tx(
    State(state): State<SharedState>,
    Json(tx): Json<Transaction>,
) -> Result<String, (StatusCode, String)> {
    let mut s = state.lock().await;
    match s.apply_tx(&tx) {
        Ok(()) => Ok("tx applied".to_string()),
        Err(e) => Err((StatusCode::BAD_REQUEST, format!("{:?}", e))),
    }
}
```

axum's `IntoResponse` is implemented for `(StatusCode, String)` to produce an HTTP response with that status and body.

### Status code conventions

| Code | Meaning | When to use |
|------|---------|-------------|
| 200 OK | Success | Normal happy path |
| 201 Created | Resource created | After POST that creates something |
| 400 Bad Request | Client sent bad data | Validation failures, business rule violations |
| 401 Unauthorized | Not authenticated | Missing/bad token |
| 403 Forbidden | Authenticated but not allowed | Permission denied |
| 404 Not Found | Resource doesn't exist | Account ID not in state |
| 409 Conflict | Concurrent modification | Two writers, one fails |
| 500 Internal Server Error | Server bug | Unexpected panic, DB down |

For our `submit_tx`:
- Invalid signature → 400 (client gave bad data)
- State root mismatch → 400 (client computed wrong state)
- Database connection lost → 500 (server problem)

## Error Pattern Maturity Levels

### Level 1: `String` errors (what we have now)
```rust
Err((StatusCode::BAD_REQUEST, format!("{:?}", e)))
```
- Pros: Easy to start
- Cons: No structured errors, hard to test client-side

### Level 2: Typed error response
```rust
#[derive(Serialize)]
struct ErrorResponse {
    code: String,
    message: String,
}

Err((StatusCode::BAD_REQUEST, Json(ErrorResponse {
    code: "INSUFFICIENT_BALANCE".to_string(),
    message: format!("{:?}", e),
})))
```
- Clients can match on `code` programmatically.

### Level 3: Custom error type implementing `IntoResponse`
```rust
impl IntoResponse for RollupError {
    fn into_response(self) -> Response {
        let (status, code) = match self {
            RollupError::AccountNotFound { .. } => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            RollupError::InsufficientBalance { .. } => (StatusCode::BAD_REQUEST, "INSUFFICIENT_BALANCE"),
            RollupError::InvalidSignature => (StatusCode::UNAUTHORIZED, "INVALID_SIG"),
            // ...
        };
        // build the response
    }
}

// handler signature gets cleaner:
async fn submit_tx(...) -> Result<String, RollupError> {
    s.apply_tx(&tx)?;  // ? converts RollupError to error response automatically
    Ok(...)
}
```
This is the production-grade pattern. Skip it until you feel the pain of Level 1.

## Sending Requests with curl

### POST with JSON body

```bash
curl -X POST http://localhost:3000/tx \
  -H 'Content-Type: application/json' \
  -d '{"from":1,"to":2,"amount":30,"nonce":1,"proof":{"r":[0],"z":[0]},"challenge_e":[0]}'
```

Components:
- `-X POST` — HTTP method
- `-H 'Content-Type: application/json'` — required so axum knows to parse as JSON
- `-d '...'` — request body

### Other useful flags

```bash
curl -i ...     # show response headers
curl -v ...     # verbose, show request and response
curl -s ...     # silent (no progress bar)
curl -o file ... # save response body to file
```

### Why `Content-Type: application/json` matters

axum's `Json<T>` extractor checks this header. Without it, axum returns a 415 Unsupported Media Type, never reaching your handler.

## Testing the Full Flow

```bash
# 1. Check server is alive
curl http://localhost:3000/health
# -> ok

# 2. Inspect initial state
curl http://localhost:3000/state-root
# -> some big number
curl http://localhost:3000/balance/1
# -> 100

# 3. Create a new account
curl -X POST http://localhost:3000/accounts \
  -H 'Content-Type: application/json' \
  -d '{"id":5,"balance":1000,"pubkey":[99]}'
# -> account 5 created

curl http://localhost:3000/balance/5
# -> 1000

# 4. Submit a (fake) tx — should fail with InvalidSignature
curl -X POST http://localhost:3000/tx \
  -H 'Content-Type: application/json' \
  -d '{"from":1,"to":2,"amount":30,"nonce":1,"proof":{"r":[0],"z":[0]},"challenge_e":[0]}'
# -> InvalidSignature
```

The full pipeline (request parsing → state lock → ZKP logic → response) is now exercised.

## Common Pitfalls

### 1. Forgetting to derive `Deserialize`
```rust
struct Transaction { /* ... */ }  // no derive!

async fn submit_tx(Json(tx): Json<Transaction>) { ... }
// Compile error: Transaction does not implement DeserializeOwned
```
Always derive `Deserialize` on types used in `Json<T>`.

### 2. Field name mismatch
JSON keys must match Rust field names. To override, use `#[serde(rename = "...")]`:

```rust
#[derive(Deserialize)]
struct CreateAccountRequest {
    id: u32,
    #[serde(rename = "initial_balance")]
    balance: u64,
}
```

### 3. Order of extractors matters
```rust
// ❌ Json must come last (it consumes the body)
async fn handler(Json(body): Json<Foo>, State(s): State<MyState>) {}

// ✅
async fn handler(State(s): State<MyState>, Json(body): Json<Foo>) {}
```
Multi-byte body extractors like `Json`, `Form` consume the body, so they must come last.

### 4. Returning the wrong type
```rust
async fn handler() -> Json<MyType> { ... }  // returns JSON
async fn handler() -> String { ... }         // returns plain text
async fn handler() -> StatusCode { ... }     // returns just a status
```

axum picks the response builder based on your return type's `IntoResponse` impl.

## What's Next

Possible directions:
- **Client binary** — write a `cargo run --bin client` that signs and POSTs valid transactions
- **Custom error responses** — implement `IntoResponse` for `RollupError` (level 3 above)
- **Persistence** — save state to disk so restarts don't wipe everything
- **Auth middleware** — verify a header before letting requests through
- **Background tasks** — periodic batch builder via `tokio::spawn`

Each is a real-world Rust pattern.
