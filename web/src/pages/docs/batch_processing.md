---
layout: ../../layouts/Layout.astro
title: "Phase 5: Batch Processing"
---

# Phase 5: Batch Processing

Combining multiple transactions into a single batch — the core abstraction of any rollup.

## What We Built

### `Batch` struct
```rust
pub struct Batch {
    pub txs: Vec<Transaction>,
    pub state_root_before: BigUint,
    pub state_root_after: BigUint,
}
```

### `apply_batch` method
```rust
pub fn apply_batch(&mut self, batch: &Batch) -> Result<(), String> {
    // 1. Sanity check: starting root matches current state
    if self.state_root() != batch.state_root_before {
        return Err("incorrect state root".to_string());
    }

    // 2. Snapshot for rollback
    let snapshot = self.accounts.clone();

    // 3. Apply each tx; rollback on failure
    for tx in &batch.txs {
        if let Err(e) = self.apply_tx(tx) {
            self.accounts = snapshot;
            return Err(e);
        }
    }

    // 4. Final check: result matches prover's claim
    if self.state_root() != batch.state_root_after {
        self.accounts = snapshot;
        return Err("state root mismatch".to_string());
    }

    Ok(())
}
```

## Key Design Decisions

### 1. Why `state_root_after` is in the batch

The prover and verifier are **separate parties**. The prover already ran the transactions on their end and knows the result. They submit `state_root_after` as a **claim**:

> "After these txs, the state will be 0xABC."

The verifier then checks the claim. This becomes the basis of trust:

```
Prover (off-chain):
  Run tx1, tx2, ... → root_after = 0xABC
  Submit batch = { txs, root_before, root_after = 0xABC }

Verifier (on-chain):
  Receive batch
  Run the same txs
  Compare own result with prover's 0xABC
  ✅ match → accept    ❌ mismatch → reject
```

### 2. Atomicity via snapshot

If any tx in the batch fails, the entire batch must be rolled back. Otherwise we'd have inconsistent state ("3 of 5 txs applied").

We do this by cloning `self.accounts` at the start. On any failure path, we restore from the snapshot.

This pattern matches database transactions: **all-or-nothing**.

### 3. Why two state-root checks?

- **Before**: ensure the prover is operating on the right starting state (not stale)
- **After**: ensure the prover's claim is correct

Both checks are essential. Without "before", a prover could submit a batch built on outdated state. Without "after", the verifier has no way to know if the prover's submitted final state is right.

## Why This Is Not Yet a "Real ZK Rollup"

Our `apply_batch` works, but it's actually closer to an **optimistic rollup** model:

| Property | Our impl | Real ZK Rollup |
|----------|----------|----------------|
| Verifier work | Re-runs all txs | Verifies a single SNARK proof |
| Verifier complexity | O(n) | O(1) |
| Prover output | Plaintext txs + claimed root | SNARK proof of correctness |
| Trust model | Verifier re-computes | Verifier trusts proof if valid |

To upgrade to a real ZK rollup, we'd need to:
1. Compile `apply_batch` logic into an arithmetic circuit
2. Prover generates a SNARK proof: "I ran this circuit, produced state_root_after from state_root_before"
3. Verifier checks the proof in milliseconds, no re-computation needed

This is what zkSync, StarkNet, Polygon zkEVM do under the hood.

## The Cost-Benefit of ZK Rollups

```
Per-tx cost on-chain:
  Pure execution: ~10,000 gas
  ZK rollup:      ~10–50 gas (verify portion of one batch proof)

Prover-side cost (off-chain):
  Pure execution: O(n)
  SNARK proving:  significantly higher (minutes to hours)
```

The genius is: **shift compute from on-chain (expensive) to off-chain (cheap)**. Prover absorbs the cost; chain stays cheap to verify.

## Rust Lessons

### `Vec<T>::clone()` for snapshots

Cheap, ergonomic atomic state pattern:
```rust
let snapshot = self.accounts.clone();
// ... try mutations ...
self.accounts = snapshot;  // rollback
```

Works because `HashMap<u32, Account>` derives `Clone` (via `Account: Clone`). No need for explicit transaction APIs.

### `if let Err(e) = ...` pattern

Cleaner than `match` for "do something if error":
```rust
if let Err(e) = self.apply_tx(tx) {
    self.accounts = snapshot;
    return Err(e);
}
```

### Shared message-bytes serialization

Both `apply_tx` (verifier side) and `sign_tx` (prover side) must produce identical bytes for the Fiat-Shamir hash. We solved this by extracting `Transaction::message_to_bytes()`. Lesson: **single source of truth for serialization** — wire formats must match exactly.

### Borrow vs ownership for `Batch`

`apply_batch(&mut self, batch: &Batch)` — borrow the batch (don't consume). The function reads from it, doesn't need ownership.

## Test Coverage Added

`test_apply_batch`:
1. Build initial state with 4 accounts
2. Construct 2 transactions, sign each
3. Run them in a "simulator" state to compute the expected `state_root_after`
4. Build the batch and call `apply_batch`
5. Assert: balances correct, state root matches, nonces incremented

This test exercises every layer: signing, transaction logic, Merkle root computation, and rollback semantics.

## Where Next?

We've built the full skeleton of a rollup. Three reasonable next steps:

1. **Range proofs in production paths** — currently we use plaintext balance check. Could use Ex3's bit-decomposition to prove `balance >= amount` without revealing amount.
2. **Real SNARK** — using arkworks or Halo2 to actually generate a proof of `apply_batch` correctness.
3. **DSL exploration** — write the same logic in Circom or Noir to feel the difference.
