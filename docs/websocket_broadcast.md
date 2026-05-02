# WebSockets, Broadcast Channels, and Fan-Out

How the server pushes a single event to many connected clients without copying code or guessing who's listening.

## The Setup

We added one endpoint:

```
GET /ws/mempool   ← upgrades to a WebSocket connection
```

When the background batch task finishes a tick, it broadcasts a message:

```
"batch applied 2/3 txs"
```

Every connected WebSocket client (terminal `websocat`, browser, mobile app, dashboard...) receives the message simultaneously. No client polls. No request-per-event. One write fans out to all.

## Why Not HTTP Polling?

Naive way to "show batch events live":

```
client: GET /mempool/events?since=t   every 500ms
server: respond with new events
```

Problems:
- **Latency** = polling interval (500ms = users see laggy data)
- **Server load** scales with `clients × pollings_per_second`, even when nothing is happening
- **Wasted requests** when there are no new events
- **No "real" concept of subscription** — server doesn't know who's listening

WebSockets flip the model: client connects once, server pushes when there's something to say.

## The Two Channels in Our App

We use **two different channel types** for two different fan-in/fan-out patterns:

### MPSC — many producers, one consumer

```
HTTP /tx handler ─┐
HTTP /tx handler ─┼─► [mpsc] ─► background batch task
HTTP /tx handler ─┘
```

This is the **mempool**: anyone can submit, one task drains.

### Broadcast — one producer, many consumers

```
                                 ┌─► WebSocket client A
background task ─► [broadcast] ──┼─► WebSocket client B
                                 └─► WebSocket client C
```

This is the **event stream**: one batch finishes, all watchers see it.

| Pattern | Type | Use case |
|---------|------|----------|
| Funnel (N→1) | `tokio::sync::mpsc` | Work queue, mempool, log aggregator |
| Fan (1→N) | `tokio::sync::broadcast` | Event bus, pub/sub, live updates |

## Broadcast Channel Mechanics

```rust
let (events_tx, _) = broadcast::channel::<String>(100);
//                                                ^ capacity
```

- `events_tx: broadcast::Sender<T>` — send messages here. **One sender** typically (you can clone it).
- The `_` is a discarded `Receiver` — `broadcast::channel` returns one to satisfy "must have a subscriber to drop", but real subscribers come later via `tx.subscribe()`.
- Capacity 100 is the **ring buffer size**. If a slow client falls more than 100 messages behind, they get `RecvError::Lagged(n)` and the oldest messages drop.

### Subscribing

```rust
let rx = events_tx.subscribe();  // each subscriber gets their own Receiver
```

`subscribe()` is cheap and unlimited — you can do it inside every WebSocket handler, every long-lived background task, every dashboard worker.

### Sending

```rust
let _ = events_tx.send("batch applied".to_string());
```

`send` returns `Result`:
- `Ok(n)` — n active subscribers got the message
- `Err(SendError(...))` — no subscribers exist (the message is dropped on the floor)

We `let _ = ...` because "no listeners" is fine for our use case.

### Receiving

```rust
match rx.recv().await {
    Ok(msg) => { /* use msg */ }
    Err(broadcast::error::RecvError::Lagged(n)) => {
        // we fell behind by n messages; they were dropped
    }
    Err(broadcast::error::RecvError::Closed) => {
        // sender was dropped; no more messages ever
        break;
    }
}
```

## WebSocket Upgrade in axum

WebSocket starts as an HTTP request:

```
GET /ws/mempool HTTP/1.1
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Key: ...
```

The server responds with `101 Switching Protocols`, and from there it's a bidirectional binary/text stream.

### The handler

```rust
async fn ws_mempool(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let rx = state.events_tx.subscribe();
    ws.on_upgrade(move |socket| async move {
        ws_loop(socket, rx).await;
    })
}
```

What happens:
1. Client sends an HTTP `GET /ws/mempool` with upgrade headers
2. axum sees the `WebSocketUpgrade` extractor and recognizes the upgrade pattern
3. Handler returns a special response that completes the upgrade dance
4. Once upgraded, axum spawns a **task running `ws_loop`** with the now-WebSocket connection
5. The original HTTP request is gone; we're in a long-lived TCP connection now

### The loop

```rust
async fn ws_loop(mut socket: WebSocket, mut rx: broadcast::Receiver<String>) {
    while let Ok(msg) = rx.recv().await {
        if socket.send(Message::Text(msg.into())).await.is_err() {
            break;  // client disconnected
        }
    }
}
```

- `rx.recv().await` waits for the next broadcast message
- `socket.send(Message::Text(...)).await` sends it to the WebSocket client
- If `socket.send` returns `Err`, client closed the connection; we break out and the task ends naturally

### Why this scales

Each WebSocket connection runs in its own tokio task. tokio multiplexes thousands of tasks onto a small thread pool. So:
- 10,000 connected clients ≠ 10,000 threads
- 10,000 tasks all `await`ing on `rx.recv()` use minimal CPU
- One `events_tx.send(msg)` wakes all 10,000 tasks; they each fire `socket.send` in parallel

This is the magic of async Rust + broadcast for fan-out.

## A Subtle Point: One Subscriber per Client

Inside `ws_mempool`, `events_tx.subscribe()` creates a **new** `Receiver` for each connection. This is critical:
- If you accidentally shared a single `Receiver` between connections, only one would ever receive messages (broadcast still copies, but the receiver pointer is per-instance).
- Each `subscribe()` call gives the new subscriber a "from now on" view — they don't see history before subscribing.

## Disconnection Handling

What if a client closes the browser tab?

```rust
if socket.send(Message::Text(msg.into())).await.is_err() {
    break;  // client disconnected
}
```

`socket.send` fails when the underlying TCP connection is gone. We `break`, the function returns, the spawned task ends, the `Receiver` is dropped. Clean.

What about graceful shutdown? You'd typically:
- Send a `Message::Close` from the server
- Wait for the client's `Close` reply (or timeout)

For our learning project, abrupt close is fine.

## Browser vs Terminal Client

Both are valid clients:

```bash
# Terminal
websocat ws://localhost:3000/ws/mempool
# (will print every broadcast message)
```

```js
// Browser
const ws = new WebSocket('ws://localhost:3000/ws/mempool');
ws.onmessage = e => console.log(e.data);
ws.onopen    = () => console.log('connected');
ws.onclose   = () => console.log('closed');
```

Both speak the same WebSocket protocol; both use the same broadcast subscriber. Mix and match.

## Why This Is the Same Pattern as Real Systems

| System | Producer | Channel | Consumers |
|--------|----------|---------|-----------|
| Chat room | message sender | broadcast | everyone in the room |
| Live trading | exchange engine | pub/sub | trader UIs |
| Blockchain explorer | indexer (new block) | broadcast | dashboard websockets |
| Discord/Slack | one user posting | broadcast | other users in channel |
| Kafka topic | producer | partitioned log | consumer groups |
| Redis pub/sub | one PUBLISH | broadcast | all subscribers |

What we built **is** the same pattern, just scoped to a single process. To scale across machines: replace `broadcast::channel` with Redis pub/sub or NATS, keep everything else.

## The Big Realization

Three building blocks:

1. **`tokio::spawn`** — run a task concurrently
2. **`broadcast::channel`** — fan out messages from one producer to many subscribers
3. **`WebSocketUpgrade`** — turn an HTTP request into a long-lived bidirectional connection

Compose those three and you have a real-time system. Add a few more (Redis for cross-process, JWT for auth, exponential backoff reconnect on the client side) and you have something production-grade.

## Rust Skills Reinforced

- **`broadcast::channel(capacity)`** vs `mpsc::channel(capacity)` — fan-out vs funnel
- **`tx.subscribe()`** — cheap, per-consumer
- **`rx.recv().await`** with `Lagged` / `Closed` error handling
- **`WebSocketUpgrade`** axum extractor
- **`ws.on_upgrade(closure)`** — register the post-upgrade task
- **`Message::Text(_)`** vs `Message::Binary(_)`
- **Long-lived async tasks** (no return; lifecycle ends on disconnect)

## Mental Model

> `broadcast::channel` is "publish once, deliver everywhere".
> `WebSocketUpgrade` is "turn this HTTP request into a persistent socket task."
> One producer + many WebSocket subscribers + one broadcast = a real-time system.

That mental model carries over directly to Discord, Slack, Bloomberg Terminal, and crypto exchange APIs.
