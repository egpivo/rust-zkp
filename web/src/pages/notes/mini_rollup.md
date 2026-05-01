---
layout: ../../layouts/Layout.astro
title: "Mini Rollup — Design Notes"
---

# Mini Rollup — Design Notes

A learning exercise that combines all 4 ZKP exercises (preimage, sum, threshold, membership) into a working mini rollup. The goal is to see how cryptographic primitives compose into a real system.

## What We Built

```
src/
  commitment.rs   — Pedersen commitment
  sigma.rs        — Sigma protocol (Schnorr) + Fiat-Shamir
  bits.rs         — bit decomposition (range proof prep)
  merkle.rs       — Merkle tree + membership proof
  account.rs      — Account: id, balance, nonce, pubkey
  state.rs        — State: HashMap of accounts + state_root + apply_tx
  transaction.rs  — Transaction: fields + serialization for hashing
  lib.rs          — module declarations
```

## Architecture

```
                     Off-chain (Prover side)
                     ┌──────────────────────┐
                     │ 1. Construct tx      │
                     │ 2. sign_tx → proof, e│
                     │ 3. Submit            │
                     └──────────────────────┘
                              │
                              ▼
                     On-chain (Verifier side)
                     ┌──────────────────────┐
                     │ apply_tx(tx):        │
                     │  - balance check     │
                     │  - account exists    │
                     │  - signature verify  │
                     │  - mutate state      │
                     │  - state_root update │
                     └──────────────────────┘
```

## Key Design Decisions

### 1. Validate-first, mutate-later

```rust
pub fn apply_tx(&mut self, tx: &Transaction) -> Result<(), String> {
    // ALL validation first
    let from_balance = ...;
    if from_balance < tx.amount { return Err(...); }
    if !self.accounts.contains_key(&tx.to) { return Err(...); }
    if !verify_signature() { return Err(...); }

    // ONLY THEN mutate
    self.accounts.get_mut(&tx.from).unwrap().balance -= tx.amount;
    ...
}
```

**Why**: if any check fails partway, we don't end up with half-applied state. Same principle as database transactions, blockchain consensus.

### 2. Signature binds to transaction (replay protection)

Naive Sigma signature only proves "I know the secret for this pubkey", which can be replayed for any transaction.

Fix: include the transaction message in the Fiat-Shamir hash:

```rust
e = hash(g || pubkey || r || tx.from || tx.to || tx.amount || tx.nonce)
```

Now changing any tx field changes `e`, which invalidates the proof. Each proof is single-use for that exact tx.

### 3. Nonce prevents same-tx replay

Even with tx-bound signatures, identical transactions (e.g., "Alice → Bob 30") would produce the same signature. Nonce makes each tx unique.

### 4. State stored as Merkle root

The `State` struct internally holds a `HashMap<u32, Account>`, but conceptually the "rollup state" is just the Merkle root of all accounts. Real on-chain contracts would only store this 32-byte hash.

Note: `state_root()` sorts account IDs before hashing, so HashMap iteration order doesn't affect the result.

### 5. System parameters in State

`g` and `p` are global system parameters, not per-transaction. Storing them in `State` simplifies `apply_tx` from 4 args (tx, g, p, &mut self) to 1 arg (tx).

## Mapping ZKP Exercises to Rollup Components

| Exercise | Used in Rollup as |
|----------|------------------|
| Ex1 (Preimage / Sigma) | Transaction signing — prove ownership of pubkey |
| Ex2 (Sum / Homomorphic) | Not directly used in this version, but conceptually maps to "balance is conserved across all accounts" |
| Ex3 (Threshold / Range proof) | Could be used to prove `balance >= 0` after subtraction (we currently do plaintext check) |
| Ex4 (Merkle membership) | State root represents account set; proving an account exists at some balance is a Merkle proof |

## What Real Rollups Add

- **SNARK over the entire batch** — instead of verifying each tx individually on-chain, generate one succinct proof that "all txs in the batch are valid"
- **Account-as-Merkle-leaf** with sparse Merkle trees for huge address spaces
- **EdDSA / ECDSA** instead of Schnorr-Sigma (production curves)
- **Range proofs** (Bulletproofs) for confidential balances
- **State transition function** as a circuit — the entire `apply_tx` logic compiled into an arithmetic circuit

## Rust Lessons

### Module system
- `pub mod foo;` declares — Rust looks for `foo.rs`
- `pub` controls visibility — without it, sibling modules can't see
- `use crate::foo::Bar` brings into scope
- `#[cfg(test)] mod tests` keeps tests out of release binary

### Ownership patterns we hit
- `BigUint` is not `Copy` — moves not implicit, must `clone()` to keep
- Clone at the `move` site, not later (`State::new(p.clone(), g.clone())`)
- Destructuring `let TestCtx { p, g, .. } = ...` for ergonomics
- `&self.accounts[&id]` borrows, doesn't move

### Validate-first pattern
The borrow checker forced this on us — you can't hold two `&mut` to the same HashMap. The fix (validate everything first, mutate after) turned out to be the right design anyway. Borrow checker as design teacher.

### Result<T, E> + ?
Idiomatic error propagation. Avoid `unwrap()` outside tests; use `?` to bubble errors up.

### Helper functions in tests
- `test_setup()` returns shared fixtures
- `sign_tx()` extracts repetitive proof-generation logic
- DRY without going overboard

## Test Coverage Summary (16 tests)

| File | Tests |
|------|-------|
| commitment.rs | test_commit, test_prov_sum |
| sigma.rs | test_sigma_protocol |
| bits.rs | test_to_bits, test_threshold, test_homomorphic |
| merkle.rs | test_hash_pair, test_build_tree, test_merkle_proof, test_merkle_membership |
| state.rs | test_apply_tx_success, test_insufficient_balance, test_to_account_missing, test_state_root_deterministic, test_state_root_changes_after_apply_tx |
| lib.rs | test_fiat_shamir |
