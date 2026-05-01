# CORS for Local Dev

What it is, why browsers block your API, and how to fix it for the rollup playground.

## The Setup

When you build the playground, you have two servers running:

```
Astro dev server:  http://localhost:4321  ← serves the web pages
axum API server:   http://localhost:3000  ← serves /accounts, /tx, etc.
```

The pages load at `:4321`, but their JavaScript needs to `fetch` data from `:3000`. Different ports = **different origins** in browser security terms.

## What Is an Origin?

An origin is the triple `(protocol, host, port)`. All three must match for the browser to consider it the "same origin."

| URL | Origin | Same as `localhost:4321`? |
|-----|--------|---------------------------|
| `http://localhost:4321/anything` | `http://localhost:4321` | ✅ |
| `http://localhost:3000/anything` | `http://localhost:3000` | ❌ different port |
| `https://localhost:4321` | `https://localhost:4321` | ❌ different protocol |
| `http://127.0.0.1:4321` | `http://127.0.0.1:4321` | ❌ different host (technically) |

## Why Browsers Block Cross-Origin Requests

Without restrictions, this attack works:

```
1. You log into bank.com → cookies stored
2. You visit evil.com (in another tab)
3. evil.com's JS runs: fetch('https://bank.com/transfer?to=hacker&amount=1000')
4. Browser auto-attaches your bank.com cookies
5. Bank sees a valid session, transfers money
```

This is **Cross-Site Request Forgery (CSRF)**. To prevent it, browsers enforce the **Same-Origin Policy**: JavaScript can only call APIs on the same origin by default.

Cross-origin requests still **happen** — the browser sends them — but the response is blocked from reaching your JS unless the server explicitly opts in.

## CORS Is the Opt-In

CORS = Cross-Origin Resource Sharing. It's the protocol the server uses to say "I'm okay with this origin calling me."

The browser sends:
```
Origin: http://localhost:4321
```

The server replies with:
```
Access-Control-Allow-Origin: *
Access-Control-Allow-Methods: GET, POST, OPTIONS
Access-Control-Allow-Headers: content-type
```

If those headers are present and match, the browser hands the response to your JS. Otherwise it blocks the read with the error you've probably seen:

> Access to fetch at 'http://localhost:3000/...' from origin 'http://localhost:4321' has been blocked by CORS policy.

## Why curl Doesn't Care

curl is not a browser. The same-origin policy is **enforced in browsers, by browsers**, to protect users. Server-to-server tools like curl, reqwest, fetch in Node.js don't enforce it.

This means: if `curl http://localhost:3000/accounts` works but the browser fails with "Failed to fetch", it's almost always CORS — not a server bug.

## Preflight Requests

For "non-simple" requests (POST with JSON body, custom headers, etc.) the browser sends a **preflight** `OPTIONS` request first:

```
OPTIONS /tx
Origin: http://localhost:4321
Access-Control-Request-Method: POST
Access-Control-Request-Headers: content-type
```

The server must respond with appropriate `Access-Control-Allow-*` headers before the browser will send the actual `POST`. If preflight fails, the actual request never happens.

This is why `tower-http`'s `CorsLayer` automatically handles `OPTIONS` for you.

## Fixing It in axum

Add `tower-http`:
```toml
tower-http = { version = "0.6", features = ["cors"], optional = true }
```

In `main.rs`:
```rust
use tower_http::cors::{Any, CorsLayer};

let cors = CorsLayer::new()
    .allow_origin(Any)
    .allow_methods(Any)
    .allow_headers(Any);

let app = Router::new()
    .route(...)
    // ... other routes
    .layer(cors)
    .with_state(app_state);
```

The `.layer(cors)` applies the CORS middleware to all routes.

### Order matters
`.layer(...)` should generally go **before** `.with_state(...)` and apply to all routes. axum applies middleware bottom-up, so the order is mostly intuitive: things you add later wrap things you added earlier.

## Why `Any` Is Fine for Dev But Not Production

`allow_origin(Any)` = `Access-Control-Allow-Origin: *` — any website can call your API.

For local dev, you want this — your Astro dev origin keeps changing, you might use `127.0.0.1`, `localhost`, different ports, etc. Locking it down is annoying for no benefit.

For production, you should pin specific origins:

```rust
use axum::http::HeaderValue;

let cors = CorsLayer::new()
    .allow_origin([
        "https://yoursite.com".parse::<HeaderValue>().unwrap(),
        "https://yoursite.io".parse::<HeaderValue>().unwrap(),
    ])
    .allow_methods([Method::GET, Method::POST])
    .allow_headers([CONTENT_TYPE]);
```

Why? `*` means "any website" — including a phishing site that tries to call your API on behalf of a logged-in user. Same-origin policy stops this; permissive CORS undoes it.

## Verifying CORS Is Working

```bash
# Send an OPTIONS preflight
curl -X OPTIONS \
  -H "Origin: http://localhost:4321" \
  -H "Access-Control-Request-Method: POST" \
  -H "Access-Control-Request-Headers: content-type" \
  -v http://localhost:3000/tx 2>&1 | grep -i "access-control"
```

Look for in the response:
```
< access-control-allow-origin: *
< access-control-allow-methods: *
< access-control-allow-headers: *
```

If those are present, the browser will allow the actual request through.

## Common Pitfalls

### 1. Server changes need restart
CORS configuration is set at startup. Editing `main.rs` doesn't take effect until you stop the server (Ctrl-C) and `cargo run --bin zkp` again.

### 2. The `OPTIONS` preflight passes but the actual request fails
Often means the actual response is missing `Access-Control-Allow-Origin`. Make sure the layer is applied to **all** routes, not just one.

### 3. "Failed to fetch" in browser, but curl works
Classic CORS. curl bypasses browser security; you must rely on browser DevTools' Network tab to see the real CORS error.

### 4. Cookies / credentials
If your API needs cookies (`fetch(..., { credentials: 'include' })`), `Any` won't work — you must specify a concrete origin AND set `.allow_credentials(true)`. Browsers refuse to send credentials with `*`.

## The Mental Model

> Same-origin policy is a browser-level firewall protecting users.
> CORS is the server saying "this firewall doesn't apply to me."
> If you write a server, you choose how permissive that opt-in is.

Local dev: open the firewall completely (`Any`).
Production: open it just enough for your real frontend.
