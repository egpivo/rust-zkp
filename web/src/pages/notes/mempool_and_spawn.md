---
layout: ../../layouts/Layout.astro
title: "Mempool, tokio::spawn, and Background Tasks"
---

# Mempool, `tokio::spawn`, and Background Tasks

What we built, why, and the Rust async patterns that hold it together.

## The Problem

Until this point, our server applied each transaction synchronously inside the HTTP handler:

```
HTTP /tx ─→ apply_tx ─→ persist ─→ HTTP 200
```

This is fine for a toy, but it bakes assumptions that real blockchains explicitly avoid:
- Every tx pays the full latency of state mutation + disk write
- Cannot batch transactions for efficiency
- Cannot apply order-sensitive logic (sequencer ordering, MEV protection)
- Server CPU couples to transaction submission rate

Real systems separate **submission** from **execution**. Geth, reth, StarkNet sequencer, even Bitcoin all do this with a **mempool**.

## The Pattern

```
HTTP /tx ─→ push to mempool channel ─→ HTTP 202 Accepted
                                          (immediate)

[Every 5 seconds]
        ↓
  background task
        ↓
  drain mempool
        ↓
  apply each tx
        ↓
  persist all touched accounts
        ↓
  log batch result
```

Two key separations:
- **Producer (HTTP handler)** doesn't wait for execution
- **Consumer (background task)** processes in batches, on its own schedule

## `tokio::spawn` — what it really is

`tokio::spawn` takes a future and starts it executing on the runtime, **immediately returning** a `JoinHandle`:

```rust
let handle = tokio::spawn(async {
    do_some_work().await
});
// handle is a JoinHandle<T> — like a thread handle
// you can `.await` it later to get the result, or just drop it (fire and forget)
```

Mental model: it's like starting a thread, but ultra-cheap (a task is bytes; a thread is megabytes). The task runs concurrently with whoever called `spawn`.

### `spawn` vs `await`

| | `spawn(future)` | `future.await` |
|---|---|---|
| Behavior | Start running in background; return handle | Pause current task until future is done |
| Concurrency | Now you have 2 tasks running concurrently | Sequential, no new task |
| Return | `JoinHandle<T>` | `T` (the future's output) |

In our mempool:
```rust
tokio::spawn(async move {
    loop { ... }   // runs forever, parallel to main
});
// main continues to axum::serve
axum::serve(listener, app).await.unwrap();   // also runs forever
```

Without `spawn`, the loop would block; axum would never start.

### `move` keyword in the spawn closure

```rust
let bg_state = app_state.clone();
let mut bg_rx = mempool_rx;

tokio::spawn(async move {
    // can use bg_state, bg_rx by VALUE
});
```

`async move { ... }` says "this future owns the captured variables." Required because:
1. The spawned task may outlive the function it was spawned from
2. Borrowing locals isn't allowed across task boundaries (lifetime can't be guaranteed)

So we **clone Arc/Senders/owned data** and `move` them in.

## MPSC Channel — `tokio::sync::mpsc`

> "Multi-Producer, Single-Consumer"

```rust
let (tx, rx) = mpsc::channel::<Transaction>(1000);
//                                            ^ capacity
```

| Property | Behavior |
|----------|----------|
| `tx: Sender<T>` | Cloneable. Multiple handlers can each hold a clone. |
| `rx: Receiver<T>` | NOT cloneable. Exactly one consumer. |
| `tx.send(value).await` | Async; blocks if channel is full (backpressure) |
| `rx.recv().await` | Async; blocks until a value or all senders dropped |
| `rx.try_recv()` | Non-blocking; returns `Err(Empty)` if nothing there |

### Why MPSC and not unbounded?

`mpsc::channel(N)` is bounded. If you try `send` when full, the `await` parks you until space. This is **backpressure** — deliberate flow control.

`mpsc::unbounded_channel()` exists, but it's dangerous: malicious clients can spam, queue grows until OOM. Always prefer bounded.

### Why MPSC and not broadcast?

- **MPSC**: each value is consumed by exactly one consumer. Right for work queues.
- **Broadcast**: each value goes to all consumers. Right for pub-sub.

For mempool: each tx should be applied once, not by every observer. MPSC.

## The Drain Pattern

```rust
let mut txs = Vec::new();
while let Ok(tx) = bg_rx.try_recv() {
    txs.push(tx);
}
```

Why `try_recv` and not `recv`?
- `recv().await` blocks until a tx arrives
- We want to process *whatever's currently queued* and stop
- `try_recv()` returns immediately with `Err(Empty)` when drained
- The `while let Ok(...)` pattern auto-stops on the first error

This pattern produces "everything queued at this moment" — perfect for batch processing.

## Avoiding Borrow Checker Conflicts in Async

When iterating and mutating state under a lock:

```rust
// ❌ Doesn't compile — `&s.accounts[...]` borrows from `s` which is moving
for tx in &txs {
    s.apply_tx(tx)?;  // mutable borrow
    storage.save_accounts(&[&s.accounts[&tx.from], &s.accounts[&tx.to]])?;  // immutable borrow simultaneously
}
```

The mutex guard `s` can't have a mutable and immutable borrow at the same time, even if logically they don't overlap.

### Fix: clone what you need

```rust
for tx in &txs {
    if s.apply_tx(tx).is_ok() {
        let from = s.accounts[&tx.from].clone();   // copy out
        let to = s.accounts[&tx.to].clone();
        storage.save_accounts(&[&from, &to]).ok();
    }
}
```

`clone()` looks expensive but `Account` is small (~32 bytes), so it's negligible. The borrow checker is happy because each borrow is short-lived and ends before the next mut borrow.

### Alternative: collect IDs, do mutation, then look up

```rust
let touched_ids: Vec<u32> = txs.iter().map(|t| [t.from, t.to]).flatten().collect();
for tx in &txs {
    s.apply_tx(tx).ok();
}
// release lock, then save
let snapshots: Vec<Account> = touched_ids.iter()
    .map(|id| s.accounts[id].clone())
    .collect();
drop(s);
storage.save_accounts(&snapshots.iter().collect::<Vec<_>>()).ok();
```

More structurally correct (lock held shorter) but more code. Pick whichever fits.

## Periodic Tick

```rust
use tokio::time::{interval, Duration};
let mut tick = interval(Duration::from_secs(5));

loop {
    tick.tick().await;   // wait for next tick
    // ... do work ...
}
```

`interval` produces a stream of evenly-spaced ticks. The first tick fires *immediately*, subsequent ticks every 5 seconds.

If your work takes 6 seconds and tick is 5, the next tick is "missed" but you get a "burst" — interval catches up. To avoid this, use `MissedTickBehavior::Delay`:

```rust
let mut tick = interval(Duration::from_secs(5));
tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
```

For our mempool, default behavior is fine.

## Why This Matters for Blockchain

Sequencer architecture is exactly this:

```
Users → /submit_tx → Mempool → Sequencer
                                  ↓
                            Order + Batch
                                  ↓
                          Apply state transitions
                                  ↓
                      Generate ZK proof (in real ZK rollups)
                                  ↓
                          Post to L1
```

Our `apply_tx` step is what real ZK rollups would replace with:
- A circuit that takes (state_root_before, txs, state_root_after) as input
- Prover generates a SNARK proof of the state transition
- The L1 contract verifies the SNARK in O(1) instead of re-executing all txs

Same Mempool → batch → execute structure, just replacing direct execution with proven execution.

## Limitations We Haven't Addressed

For learning purposes our mempool is intentionally simple. Real systems handle:

| Concern | What we do | Real systems |
|---------|-----------|--------------|
| Tx ordering | FIFO from channel | Fee-based priority, MEV considerations |
| Duplicate detection | None | Tx hash dedup |
| Memory bound | 1000 channel capacity | KB-bound + eviction |
| Crash recovery | Lost on restart | Persisted mempool / WAL |
| Backpressure feedback | Send blocks silently | HTTP 429 Too Many Requests |
| Fairness | Single-thread FIFO | Per-account rate limits |

These are good follow-up exercises but not required for understanding the pattern.

## Rust Skills Reinforced

- **`tokio::spawn`** — fire-and-forget background task
- **`async move {}`** closure — own captured values
- **`Arc<T>::clone()`** for sharing across tasks (cheap; just a counter increment)
- **`mpsc::channel`** — bounded async producer-consumer
- **`Sender::send().await`** — async with backpressure
- **`Receiver::try_recv()`** — non-blocking drain
- **Mutex + clone trick** — avoid simultaneous mut/immut borrows under a lock
- **`tokio::time::interval`** — periodic timer
- **`Duration::from_secs(5)`** — `std::time` re-exported in tokio docs

## Mental Model

> `spawn` = "run this concurrently, I'll continue without it."
> Channel = "send work from over there to over here."
> Receiver = "the one place that processes the work."
> Loop with tick = "wake up every N seconds and clear the inbox."

Compose those four pieces and you have a sequencer.
