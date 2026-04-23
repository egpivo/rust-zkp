# Ex3: Prove Threshold Check (v >= T)

## ZKP Four Elements

| Element | Content |
|---------|---------|
| **Witness** | `v`, `delta = v - T`, bits of `delta`, and blinding factors |
| **Public Input** | `T`, `C_v` (commitment of v), `C_bits` (commitments of each bit), `r_delta` |
| **Relation** | `v >= T` (equivalently, `delta = v - T` is non-negative) |
| **What Verifier Checks** | `C_v == commit(T + delta)` AND `product of C_bits^(2^i) == commit(delta, r_delta)` |

## Why Inequality is Hard

Previous exercises used equality (Ex1: `hash(w) = x`, Ex2: `a + b + c = S`). But the real world is full of inequalities:
- Balance ≥ 0
- Age ≥ 18
- Collateral ≥ loan

**Problem**: modular arithmetic has no notion of "greater than". In `mod p`, numbers wrap around — there's no "size".

**Solution**: **bit decomposition**. If `delta` can be written as a sum of bits (each 0 or 1), then `delta` is non-negative. This is the foundation of **range proofs** (Bulletproofs, zk-rollup balance checks).

## Core Algorithm

### 1. Prover computes
```
delta = v - T           // must be >= 0, else underflow panics
bits = [b_0, b_1, ..., b_{n-1}]  // binary representation of delta
```

### 2. Commit each bit
```
for each bit b_i with random r_i:
    C_i = commit(b_i, r_i) = g^b_i * h^r_i mod p
```

### 3. Choose r_delta carefully
```
r_delta = Σ r_i * 2^i
```
This is crucial — it makes the homomorphic check work (see below).

### 4. Verifier checks
```
product = C_0^1 * C_1^2 * C_2^4 * ... * C_{n-1}^(2^{n-1}) mod p
       = g^(Σ b_i * 2^i) * h^(Σ r_i * 2^i)
       = g^delta * h^r_delta
       = commit(delta, r_delta)
```

If the equation holds, `delta` really is the bit combination, and `v = T + delta`.

## Why It's Zero-Knowledge

- Verifier never sees individual `b_i` — only their commitments `C_i`
- Hiding property of Pedersen: `C_i` reveals nothing about `b_i`
- Verifier only learns: "delta is representable as 8 bits" → i.e., `0 <= delta < 2^8` → hence `v >= T`

## What's NOT Complete

Our simplified version is missing one crucial piece: **proof that each `b_i ∈ {0, 1}`**.

Without this, a cheating prover could commit arbitrary values like `b_0 = 100`, and the algebra would still work out. A full range proof (e.g., Bulletproofs) proves each bit is binary via an additional Sigma protocol on `b_i * (1 - b_i) == 0`.

For our learning purposes, we accept this gap.

## Rust Lessons

### 1. `Vec<T>` construction patterns

```rust
// Fill with default value
let vecs = vec![BigUint::from(0u32); num_bits];

// Build via iterator + collect
let r_bits: Vec<BigUint> = (0..8)
    .map(|_| rng.gen_biguint_below(&p))
    .collect();

// Zip two iterators
let c_bits: Vec<BigUint> = bits.iter().zip(&r_bits)
    .map(|(b, r)| commit(b, r, &g, &h, &p))
    .collect();
```

### 2. `.bit(i)` vs manual decomposition

Idiomatic:
```rust
fn to_bits(n: &BigUint, num_bits: usize) -> Vec<BigUint> {
    (0..num_bits)
        .map(|i| if n.bit(i as u64) { BigUint::from(1u32) } else { BigUint::from(0u32) })
        .collect()
}
```

Manual (what we first wrote):
```rust
while a > BigUint::from(0u32) && i < num_bits {
    vecs[i] = &a % BigUint::from(2u32);
    a /= BigUint::from(2u32);
    i += 1;
}
```

The idiomatic version is cleaner — no `mut`, no `clone`, no manual index tracking.

### 3. Ownership & references with `BigUint`

Key pattern: **use `&` everywhere you don't want to consume**.

```rust
let lhs = (&c_a * &c_b * &c_c) % &p;  // borrow, don't move

&a + &b + &c   // this also works — BigUint implements Add for references
```

Without `&`, the variable is moved and can't be used again.

### 4. Accumulator loops

```rust
// Mutable accumulator
let mut lhs = BigUint::from(1u32);
for i in 0..8 {
    lhs = (lhs * c_bits[i].modpow(&BigUint::from(1u32 << i), &p)) % &p;
}

// Or using sum() for simple cases
let r_delta: BigUint = (0..8).map(|i| &r_bits[i] * BigUint::from(1u32 << i)).sum();
```

`sum()` works when you have a closed-form iterator; mutable loop when you need more complex per-step logic (like `% p`).

### 5. `1u32 << i` — bit shift as powers of 2

`1u32 << i` = `2^i`. Used here to compute the positional weight of each bit.

## Comparison Table

| Ex | Proof Goal | Technique | New Concept |
|---|---|---|---|
| Ex1 | Know preimage | Sigma + Fiat-Shamir | Interactive/non-interactive ZKP |
| Ex2 | Sum is correct | Homomorphic addition | Commitments can be combined |
| Ex3 | Value ≥ threshold | Bit decomposition + homomorphic | Range proof basics |
