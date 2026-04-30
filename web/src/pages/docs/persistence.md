---
layout: ../../layouts/Layout.astro
title: "Persistence with sled"
---

# Persistence with sled

Adding durable storage so the rollup state survives server restarts.

## Why Persistence

Without persistence, all state lives in memory. Every restart wipes accounts, balances, nonces, and merkle root — completely unusable for any real system. Real blockchain nodes (`geth`, `erigon`, `reth`) all persist state to disk via embedded KV databases.

## Choice of Database

We chose `sled`. Alternatives considered:

| Database | Pros | Cons |
|----------|------|------|
| `sled` | Pure Rust, zero-dep, simple API | Beta status, slower than rocksdb |
| `rocksdb` | Industry standard for blockchain (geth, erigon, reth) | C++ binding, larger build footprint |
| `redb` | Pure Rust, faster than sled | Newer, smaller ecosystem |
| `sqlite` | SQL queries, mature | Heavier, overkill for KV |

For learning purposes, `sled` was the right starting point. To upgrade to `rocksdb` later: same KV concepts, different API.

## Key Design

Sled is a key-value store. Keys and values are arbitrary bytes; we use prefix patterns for namespacing:

| Key | Value |
|-----|-------|
| `account:1` | bincode-serialized `Account` |
| `account:42` | ... |
| `account:N` | ... |

`scan_prefix("account:")` walks all account keys — useful for loading the full state on startup.

This pattern scales to more types: `params:p`, `params:g`, `tx:hash`, `block:height`, etc. Each prefix is its own logical "table".

## Serialization: bincode

`bincode` is a binary serializer built on serde. Why not JSON?

| Format | Speed | Size | Human-readable | Use case |
|--------|-------|------|----------------|----------|
| JSON | Slow | Large | Yes | API requests, config |
| `bincode` | Fast | Small | No | Internal storage, caches |

For an internal database the user never reads directly, `bincode` wins on every dimension that matters. JSON is for crossing process boundaries.

`bincode` requires the type to derive `serde::Serialize` and `serde::Deserialize`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: u32,
    pub balance: u64,
    pub nonce: u64,
    pub pubkey: BigUint,
}
```

`BigUint` doesn't implement these by default — you must enable the `serde` feature in Cargo.toml:
```toml
num-bigint = { version = "0.4", features = ["serde"] }
```

## The `Storage` Module

```rust
use sled::Db;
use crate::account::Account;

pub struct Storage {
    db: Db,
}

impl Storage {
    pub fn open(path: &str) -> sled::Result<Self> {
        let db = sled::open(path)?;
        Ok(Self { db })
    }

    pub fn save_account(&self, account: &Account) -> sled::Result<()> {
        let key = format!("account:{}", account.id);
        let value = bincode::serialize(account).unwrap();
        self.db.insert(key, value)?;
        Ok(())
    }

    pub fn load_all_accounts(&self) -> sled::Result<Vec<Account>> {
        let prefix = "account:";
        let mut accounts = vec![];
        for item in self.db.scan_prefix(prefix) {
            let (_, value) = item?;
            accounts.push(bincode::deserialize(&value).unwrap());
        }
        Ok(accounts)
    }
}
```

Note: `&self` not `&mut self`. `sled::Db` is internally synchronized (built on `Arc`), so multiple threads can write concurrently without external locking.

## Sharing Storage Across Handlers

The server passes one `Storage` instance to every async handler. We use `Arc<Storage>`:

```rust
#[derive(Clone)]
struct AppState {
    rollup: Arc<Mutex<RollupState>>,
    storage: Arc<Storage>,
}
```

- `Arc` because handlers run on multiple async tasks; they all need read access without taking ownership
- No `Mutex<Storage>` needed because sled is internally thread-safe

### `Arc<Mutex<T>>` vs `Arc<T>` recap

| Need | Pattern |
|------|---------|
| Shared mutable state with non-thread-safe inner type | `Arc<Mutex<T>>` |
| Shared, internally thread-safe type | `Arc<T>` |

`RollupState` (HashMap) needs `Mutex`. `Storage` (sled wrapper) doesn't.

## Startup: Recovery from Disk

```rust
let storage = Arc::new(Storage::open("./data").unwrap());

let mut rollup_state = RollupState::new(p, g);
for account in storage.load_all_accounts().unwrap() {
    rollup_state.add_account(account);
}
```

On startup:
1. Open the sled database at `./data`
2. Scan all `account:*` keys
3. Deserialize each into `Account`
4. Populate the in-memory `RollupState`

The in-memory map is now a faithful reflection of disk. From this point on, any change must be written back to keep them synced.

## The Hardcoded-Account Trap

A subtle bug worth documenting:

```rust
// BAD: every restart adds these, overwriting whatever was loaded
for account in storage.load_all_accounts().unwrap() {
    rollup_state.add_account(account);
}
rollup_state.add_account(Account::new(1, 100, pubkey.clone()));
rollup_state.add_account(Account::new(2, 50, pubkey.clone()));
```

After the first run, the disk has `1: balance=70` (after some tx). Next restart:
1. Load `1: balance=70` into memory
2. Hardcoded line overwrites with `1: balance=100`
3. Persistence is silently broken

Fix: only seed if the database is empty.

```rust
if storage.load_all_accounts().unwrap().is_empty() {
    // seed initial accounts here
    storage.save_account(&Account::new(1, 100, pubkey.clone())).unwrap();
}
// Then load from disk:
for account in storage.load_all_accounts().unwrap() {
    rollup_state.add_account(account);
}
```

## Writing Back After Mutation

Memory and disk drift if writes go to one but not the other. Every handler that mutates state must persist the change:

```rust
async fn submit_tx(
    State(state): State<AppState>,
    Json(tx): Json<Transaction>,
) -> Result<String, (StatusCode, String)> {
    let mut s = state.rollup.lock().await;
    match s.apply_tx(&tx) {
        Ok(()) => {
            // Persist both modified accounts
            state.storage.save_account(&s.accounts[&tx.from]).unwrap();
            state.storage.save_account(&s.accounts[&tx.to]).unwrap();
            Ok("tx applied".to_string())
        }
        Err(e) => Err((StatusCode::BAD_REQUEST, format!("{:?}", e))),
    }
}
```

Order matters subtly. We mutate memory first, then persist. If the persist fails, memory and disk are out of sync.

## What's NOT Solved Yet

### 1. Crash safety / atomicity
If the server crashes between `save_account(from)` and `save_account(to)`, you have inconsistent on-disk state. Fix: sled's batch API:
```rust
let mut batch = sled::Batch::default();
batch.insert("account:1", value1);
batch.insert("account:2", value2);
db.apply_batch(batch)?;  // atomic write
```

### 2. Migrations
If `Account` ever gains a new field, deserializing old data may fail. Solutions:
- Versioned types (`Account_v1`, `Account_v2`, with migration logic)
- `serde(default)` for new fields with sensible defaults

### 3. State root in storage
Currently we recompute the merkle root every time. For large states, persisting it (with invalidation logic on changes) saves CPU.

### 4. Garbage collection / pruning
Old sled data grows over time. Real systems run periodic compaction.

### 5. Backup / recovery
A serious system needs snapshots, replication, or write-ahead logs for disaster recovery.

These are all real backend topics. We're skipping them for the learning project.

## Test Plan

End-to-end persistence test:

```bash
# Terminal 1
cargo run --bin zkp

# Terminal 2
cargo run --bin cli -- register --id 100 --balance 1000 --secret 12345
cargo run --bin cli -- register --id 200 --balance 0 --secret 67890
cargo run --bin cli -- send --from 100 --to 200 --amount 30 --nonce 1 --secret 12345
cargo run --bin cli -- balance 100   # 970
cargo run --bin cli -- balance 200   # 30
```

Now restart the server (Ctrl-C, then `cargo run --bin zkp` again).

```bash
# Terminal 2
cargo run --bin cli -- balance 100   # still 970
cargo run --bin cli -- balance 200   # still 30
```

The persistence layer is working when this passes.

## Rust Lessons

### `Arc<T>` for shareable thread-safe types
When the inner type is already `Sync`, just wrap in `Arc`. No `Mutex` needed.

### `Result<T, E>` from external crates
`sled::Result<T>` is `Result<T, sled::Error>`. Convert with `?` to propagate, or `.unwrap()` if you know it can't fail (or accept the panic).

### Trait derives chain through generics
`#[derive(Serialize, Deserialize)]` on `Account` only works if every field is also `Serialize + Deserialize`. That's why we needed the `serde` feature on `num-bigint`.

### Owned vs borrowed in serialization
- `bincode::serialize(&account)` — borrows, returns `Vec<u8>`
- `bincode::deserialize(&bytes)` — borrows bytes, returns owned struct

You don't transfer ownership; you copy data into/out of the binary representation.

### Path safety with `format!`
```rust
let key = format!("account:{}", account.id);
```
Easy and clear. For higher safety, you'd use a typed key with explicit encoding, but `format!` is the standard quick approach.
