# rust-zkp

A learning project that builds a minimal ZK-Rollup from scratch in Rust — no SNARK frameworks, just the cryptographic primitives composed by hand.

The goal isn't production-grade crypto; it's understanding **how every layer works** by writing it.

## What's Inside

A working vertical slice of a ZK-Rollup:

- **ZKP primitives**: Pedersen commitment, Sigma protocol, Fiat-Shamir transform, bit-decomposition range proofs, Merkle membership proofs
- **State machine**: account model with balances, nonces, and pubkeys; Merkle root over accounts
- **Transaction layer**: Sigma-signed transactions with replay protection (nonce + tx-bound challenges)
- **Batch processing**: atomic apply-many-or-rollback semantics
- **HTTP API server** (axum): receives transactions, exposes state queries
- **CLI client** (clap + reqwest): signs transactions, talks to the server
- **Persistence** (sled): atomic batch writes, recovery on restart

## Architecture

```
       Client (CLI)                       Server (Node)
  ─ holds secret                    ─ holds state
  ─ builds unsigned tx              ─ accepts signed tx
  ─ signs (Sigma + Fiat-Shamir)     ─ verifies signature
  ─ POST /tx        ─────────────►  ─ applies tx atomically
                                    ─ persists to sled
                                    ─ updates merkle root
```

## Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/health` | Liveness check |
| GET | `/params` | System parameters (p, g) |
| GET | `/state-root` | Current merkle root |
| GET | `/balance/:id` | Account balance |
| POST | `/accounts` | Register account |
| POST | `/tx` | Submit signed transaction |

## Running It

### Server
```bash
cargo run --bin zkp
# Listening on http://0.0.0.0:3000
```

### Client
```bash
# Inspect state
cargo run --bin cli -- state-root
cargo run --bin cli -- balance 100

# Register accounts
cargo run --bin cli -- register --id 100 --balance 1000 --secret 12345
cargo run --bin cli -- register --id 200 --balance 0 --secret 67890

# Send a transaction (signed end-to-end)
cargo run --bin cli -- send --from 100 --to 200 --amount 30 --nonce 1 --secret 12345

# Verify
cargo run --bin cli -- balance 100   # 970
cargo run --bin cli -- balance 200   # 30

# Persistence works:
# Ctrl-C the server, restart it; balances stay.
```

## Tests

```bash
cargo test
# 18 passed; 0 failed
```

Each ZKP primitive and the rollup logic are unit-tested.

## Project Layout

```
src/
  commitment.rs   — Pedersen commitment
  sigma.rs        — Sigma protocol + Fiat-Shamir
  bits.rs         — bit decomposition (range proof)
  merkle.rs       — Merkle tree + membership proofs
  account.rs      — Account struct
  transaction.rs  — Transaction + serialization for signing
  state.rs        — RollupState (HashMap of accounts + merkle root)
  batch.rs        — Batch struct
  storage.rs      — sled-backed persistence
  error.rs        — RollupError + IntoResponse
  main.rs         — axum HTTP server
  bin/
    cli.rs        — clap-based CLI client
docs/
  ex1_preimage.md
  ex2_sum.md
  ex3_threshold.md
  ex4_membership.md
  mini_rollup.md
  batch_processing.md
  persistence.md
  backend_basics.md
  backend_post_endpoints.md
  rust_async_primer.md
  zkp_in_blockchain.md
```

## What This Is *Not*

The cryptography is **deliberately simplified for learning**:

| Concept | This project | Production ZK Rollup |
|---------|--------------|---------------------|
| Group | `Z*_p` with small prime (`p=223`) | Elliptic curves (BN254, BLS12-381) |
| Verification | Verifier re-runs every tx (`O(n)`) | SNARK proof verification (`O(1)`) |
| Range proof | Bit-decomposition without bit-validity proof | Bulletproofs / Plonkish lookups |
| Privacy | Basic | Shielded with proper ZK Merkle |

This is a **teaching MVP**. To use it for anything real, you'd swap in a SNARK proving system (arkworks, halo2) and proper elliptic-curve crypto.

## Documentation

The `docs/` folder has standalone walkthroughs of each concept — the math, why it works, where it breaks, and the Rust patterns used. Read them in order:

1. `ex1_preimage.md` — Sigma protocol & Fiat-Shamir
2. `ex2_sum.md` — Homomorphic commitments
3. `ex3_threshold.md` — Range proofs via bit decomposition
4. `ex4_membership.md` — Merkle membership proofs
5. `mini_rollup.md` — Composing primitives into a rollup
6. `batch_processing.md` — Atomic batches
7. `persistence.md` — sled, bincode, recovery
8. `backend_basics.md` — axum HTTP server fundamentals
9. `backend_post_endpoints.md` — JSON, serde, error responses
10. `rust_async_primer.md` — async/await/tokio mental model
11. `zkp_in_blockchain.md` — How rollups use ZKPs in practice

## Stack

- **Rust 2024 edition**
- **Crypto**: `num-bigint`, `sha2`
- **Server**: `axum`, `tokio`
- **CLI**: `clap`, `reqwest`
- **Persistence**: `sled`, `bincode`
- **Testing**: built-in `cargo test`
- **Concurrency**: `rayon` (parallel signature verification)

## Build

```bash
cargo build
cargo run --bin zkp     # server
cargo run --bin cli     # client
cargo test              # all 18 tests
```

## Status

This is a **personal learning project**, not production crypto. Use it to understand the architecture; do not build anything that handles real money on top of it.
