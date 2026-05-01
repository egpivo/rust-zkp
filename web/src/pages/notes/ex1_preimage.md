---
layout: ../../layouts/Layout.astro
title: "Ex1: Prove Knowledge of Preimage"
---

# Ex1: Prove Knowledge of Preimage

## What is a Preimage?

- Given a function `f`, the **preimage** of output `y` is the input `x` such that `f(x) = y`. In our case, `f = g^x mod p`.
- **Preimage resistance**: it should be computationally infeasible to find `x` given only `y`. This is what makes the proof meaningful — the verifier can't just reverse-engineer the secret from the public input.

## ZKP Four Elements

| Element | In This Exercise |
|---------|-----------------|
| **Witness** | `secret` — the private value only the prover knows |
| **Public Input** | `C = g^secret mod p` — visible to the verifier |
| **Relation** | `g^w mod p == C` — knowledge of discrete log |
| **What Verifier Checks** | proof corresponds to C, without learning secret |

## Concepts Learned

### 1. Pedersen Commitment

```
commit(v, r) = g^v * h^r mod p
```

- `v` = value to commit, `r` = random blinding factor
- **Binding**: cannot change the committed value after committing (unless you know log_g(h))
- **Hiding**: commitment reveals nothing about the original value (because of random r)
- **Homomorphic**: `commit(a) * commit(b) = commit(a+b)` (used in Ex2)

### 2. Sigma Protocol (Interactive)

Three-move protocol, two roles:

```
Prover                          Verifier
  |                                |
  |  1. R = g^k mod p              |
  |  ------- R -------->           |
  |                                |
  |  2.       <---- e --------     |  (random challenge)
  |                                |
  |  3. z = k + e * secret         |
  |  ------- z -------->           |
  |                                |
  |      check: g^z == R * C^e     |
```

**Why it works** (expand the verification equation):
```
g^z = g^(k + e * secret)
    = g^k * g^(e * secret)
    = g^k * (g^secret)^e
    = R * C^e  ✓
```

**Why it's secure**:
- Prover can't cheat: without knowing secret, can't compute a valid z
- Verifier learns nothing: z = k + e * secret, but k is random, so z leaks no info about secret

### 3. Interactive vs Non-Interactive Challenge

#### Interactive (Sigma Protocol)

Verifier picks a random `e` and sends it to the prover.

| Pros | Cons |
|------|------|
| Simple to reason about security | Requires real-time communication between prover and verifier |
| Verifier controls randomness — prover cannot manipulate `e` | Cannot produce a proof offline or share it with multiple verifiers |
| Well-studied, strong theoretical foundations | Each verification requires a new interaction |

#### Non-Interactive (Fiat-Shamir Transform)

Replace verifier's random challenge with a hash:

```
e = hash(g, C, R) mod p
```

| Pros | Cons |
|------|------|
| Proof can be generated offline and verified by anyone | Security relies on hash being a "random oracle" — a modeling assumption |
| No back-and-forth needed — single message from prover | Vulnerable if hash function is weak or inputs are not properly domain-separated |
| Proof is portable — can be posted on-chain, shared, stored | Slightly more complex to implement correctly (need careful hash input construction) |

### 4. Hash Concatenation Detail

```rust
hasher.update(g_bytes);
hasher.update(c_bytes);
hasher.update(r_bytes);
```

Equivalent to `hash(g_bytes || c_bytes || r_bytes)`, since SHA256 is a streaming compression function.

**Caveat**: raw byte concatenation has no boundary info — different-length inputs can collide:
```
g=[01,02] c=[03]     → [01,02,03]
g=[01]    c=[02,03]  → [01,02,03]  ← same hash!
```

Proper fix: add a length prefix before each segment. Not an issue in our exercise since p is fixed and byte lengths are nearly identical.

## Rust Lessons

- `BigUint::from(2u32)` — literal suffix to specify type
- `base.modpow(&exp, &modulus)` — modular exponentiation
- `&BigUint` — use references to avoid ownership transfer
- `rand::thread_rng()` + `rng.gen_biguint_below(&p)` — generate random big integers (requires `num-bigint` `rand` feature)
- `Sha256::new()` / `update()` / `finalize()` — streaming hash
- `#[cfg(test)] mod tests` — tests only compile during `cargo test`
- `cargo add`, `cargo doc --open` — package management and docs
