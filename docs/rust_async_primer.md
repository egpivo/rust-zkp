# Rust Async Primer

A focused walkthrough of Rust async/await. Read this if you're not yet comfortable with `async`, `await`, `tokio`, and the runtime model.

## Why Async?

Imagine a server handling 10,000 connections. Most of the time, each connection is **waiting** — waiting for the client to send something, waiting for a database response, waiting for a file read.

| Approach | Pros | Cons |
|----------|------|------|
| **Thread per connection** | Simple to write | OS threads are heavy (~1MB stack each); 10,000 threads = 10GB just for stacks |
| **Async tasks** | Lightweight (~bytes each); same code can serve millions of connections | Different mental model |

Async lets you write code that **looks sequential** but actually pauses when waiting and resumes when ready, all on a small thread pool.

## The Mental Model

```rust
async fn fetch_two_things() -> (Data1, Data2) {
    let a = fetch_one().await;
    let b = fetch_two().await;
    (a, b)
}
```

What happens:
1. `fetch_one()` is called, returns a `Future` (a "promise")
2. `.await` says: "pause here until that future is ready"
3. While paused, the runtime can run **other tasks**
4. When `fetch_one` completes, this task resumes from where it left off
5. Same for `fetch_two`
6. Returns the tuple

It looks like blocking sequential code but it's not blocking — the underlying thread is doing other work while we wait.

## `async fn` and `Future`

```rust
async fn foo() -> i32 { 42 }
```

This does **not** return `42`. It returns a `Future<Output = i32>`. The body runs only when something `.await`s the future.

```rust
let future = foo();          // No work happens yet!
let value = future.await;    // Now it runs, returns 42
```

This is different from JavaScript Promises (which start running immediately when called). Rust's futures are **lazy** — they don't progress without an executor.

## The Runtime: `tokio`

A runtime is the thing that actually polls and runs futures.

```rust
#[tokio::main]
async fn main() {
    // your async code
}
```

This macro expands to:

```rust
fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        // your code
    });
}
```

Key insight: **`async fn main()` is a lie**. The actual `main()` is sync. The macro creates a runtime and runs your code inside it.

### What the runtime does
- Maintains a thread pool (default: number of CPU cores)
- Schedules futures across these threads
- When a future is `.await`-ing on I/O, the runtime parks it and runs another task
- When the I/O event arrives (data received, timer fires), the runtime wakes the parked task

You write `async/await` code; the runtime does the bookkeeping.

## Common Operations

### Sequential awaits
```rust
let a = step_one().await;
let b = step_two(a).await;  // depends on a
```

### Concurrent awaits with `join!`
```rust
let (a, b) = tokio::join!(fetch_one(), fetch_two());
```
Both run concurrently — total time is `max(a, b)` not `a + b`.

### Spawning a task
```rust
let handle = tokio::spawn(async {
    do_some_work().await
});
let result = handle.await.unwrap();
```
`tokio::spawn` returns a `JoinHandle`. The task runs in the background.

### Sleep
```rust
use tokio::time::{sleep, Duration};
sleep(Duration::from_secs(2)).await;
```
Async sleep — doesn't block the thread, only this task.

## Common Pitfalls

### 1. Blocking inside async

```rust
async fn bad() {
    std::thread::sleep(Duration::from_secs(2));  // BLOCKS the runtime thread!
}
```

This blocks the **entire thread**, freezing all other tasks scheduled on it. Use `tokio::time::sleep` instead, or move heavy CPU work to `spawn_blocking`:

```rust
let result = tokio::task::spawn_blocking(|| {
    expensive_cpu_work()  // okay to block here
}).await.unwrap();
```

### 2. Forgetting to `.await`

```rust
async fn bad() {
    foo();  // returns a future but is never awaited — body never runs!
}
```
The compiler warns about unused futures, but it's an easy mistake.

### 3. Lifetime issues with `&` across `.await`

```rust
async fn bad(s: &str) {
    do_something().await;
    println!("{}", s);  // s might not live long enough
}
```
The reference `s` must live across the `.await` point. Fix: use owned data, `Arc<T>`, or constrain lifetimes.

### 4. `std::sync::Mutex` vs `tokio::sync::Mutex`

| | std | tokio |
|---|---|---|
| `lock()` | Blocking call | Async (`lock().await`) |
| Use case | Pure sync code | Inside async functions |
| Holding across `.await` | Bad — blocks runtime | Fine |

In an async server, **always** use `tokio::sync::Mutex`.

## Sync vs Async Cheat Sheet

```rust
// Sync version
fn read_file(path: &str) -> String {
    std::fs::read_to_string(path).unwrap()
}

// Async version
async fn read_file(path: &str) -> String {
    tokio::fs::read_to_string(path).await.unwrap()
}
```

The structure is identical. Differences:
- `async fn` instead of `fn`
- `tokio::fs` instead of `std::fs`
- `.await` after the I/O call

## Why `Arc<Mutex<T>>` for shared state in async?

In axum:
```rust
type SharedState = Arc<Mutex<RollupState>>;

async fn handler(State(state): State<SharedState>) {
    let s = state.lock().await;
    // ...
}
```

- Each handler runs as a task
- Multiple tasks may execute simultaneously on different threads
- They all need to access the same `RollupState`
- `Arc` lets them share ownership safely
- `Mutex` ensures only one mutates at a time

Why not just `Arc<RollupState>`? Because `Arc<T>` only allows shared **immutable** access. To mutate, you need interior mutability (`Mutex`, `RwLock`, etc).

Why not just `Mutex<RollupState>`? Because a `Mutex` lives in one place — you can't easily share a reference across many handlers. `Arc` makes sharing cheap.

The combo `Arc<Mutex<T>>` is the canonical pattern for shared mutable state across async tasks.

## When to use each pattern

| Need | Use |
|------|-----|
| Run multiple async ops concurrently, wait for all | `tokio::join!` |
| Run one in background while continuing | `tokio::spawn` |
| Run a CPU-heavy sync function from async | `tokio::task::spawn_blocking` |
| Share read-only data | `Arc<T>` |
| Share mutable data | `Arc<Mutex<T>>` or `Arc<RwLock<T>>` |
| Many readers, occasional writer | `Arc<RwLock<T>>` |
| Async-aware lock | `tokio::sync::Mutex` not `std::sync::Mutex` |
| Channel between tasks | `tokio::sync::mpsc` |

## Putting It Together: The axum Server

```rust
#[tokio::main]
async fn main() {
    // 1. Set up runtime (via macro)

    // 2. Build state shared across handlers
    let shared = Arc::new(Mutex::new(state));

    // 3. Build router
    let app = Router::new()
        .route("/foo", get(handler))
        .with_state(shared);

    // 4. Bind socket and serve forever
    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn handler(State(s): State<SharedState>) -> String {
    let guard = s.lock().await;
    guard.something().to_string()
}
```

Every part of this code maps to a concept above:
- `#[tokio::main]` → runtime setup
- `Arc<Mutex<T>>` → shared mutable state
- `async fn handler` → returns `Future`
- `.lock().await` → async lock acquisition
- `.serve(listener, app).await` → run the server, suspending until shutdown

## Suggested Mental Model When Reading Async Code

When you see `.await`:
1. "This task pauses here"
2. "The runtime can run other tasks now"
3. "When the awaited thing is ready, this task resumes"

When you see `async fn`:
1. "This is a recipe, not an action"
2. "Calling it makes a `Future`; awaiting it runs the recipe"

When you see `tokio::spawn`:
1. "Start a separate task that runs concurrently"
2. "Returns a handle to await its result later"

That's enough mental model to read 90% of async Rust code.

## Short sequence + tiny example

Short timeline when a task awaits I/O:
1. Future.poll sees I/O not ready → returns Pending and registers a Waker/interest with the runtime.
2. Runtime registers that interest with the kernel (epoll/kqueue/IOCP/io_uring) and removes the task from the ready queue.
3. Executor runs other tasks on worker threads (no thread blocking).
4. Kernel notifies completion → runtime calls Waker.wake() to requeue the task.
5. Executor polls the task again and it resumes (Ready).

One-line ASCII summary:
poll -> Pending + register interest -> kernel notifies -> Waker.wake() -> poll -> Ready

Tiny tokio example (condensed):

```rust
use tokio::net::TcpListener;
use tokio::io::AsyncReadExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:4000").await?;
    loop {
        let (mut socket, _) = listener.accept().await?; // registers accept interest
        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            if let Ok(n) = socket.read(&mut buf).await { // registers read interest
                println!("got {} bytes", n);
            }
        });
    }
}
```

Note: runtime registers I/O interest with the kernel (not the CPU); keep long CPU work in `spawn_blocking`.
