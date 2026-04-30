---
layout: ../../layouts/Layout.astro
title: "Ex2: Prove Sum is Computed Correctly"
---

# Ex2: Prove Sum is Computed Correctly

## ZKP Four Elements

| Element | Content |
|---------|---------|
| **Witness** | `(a, b, c)` and blinding factors `(r_a, r_b, r_c)` |
| **Public Input** | `S` (the sum), commitments `C_a, C_b, C_c`, and `r_total` |
| **Relation** | `a + b + c == S` |
| **What Verifier Checks** | `C_a * C_b * C_c == commit(S, r_total)` without knowing a, b, c |

## Core Concept: Homomorphic Property

Pedersen commitment is **additively homomorphic** — multiplying commitments equals committing the sum:

```
C_a = g^a * h^r_a mod p
C_b = g^b * h^r_b mod p
C_c = g^c * h^r_c mod p

C_a * C_b * C_c = g^(a+b+c) * h^(r_a+r_b+r_c) mod p
                = commit(a+b+c, r_a+r_b+r_c)
                = commit(S, r_total)
```

This works because exponent rules: `g^a * g^b = g^(a+b)`.

## Homomorphic Commitment vs Homomorphic Encryption

They belong to the same "homomorphic" family — **operation on ciphertext = operation on plaintext** — but serve different purposes:

| | **Pedersen Commitment** (what we use) | **Homomorphic Encryption (HE / FHE)** |
|---|------|------|
| Purpose | Commitment — lock a value, verify later | Encryption — hide a value, compute on it |
| Supported ops | Addition only (additively homomorphic) | FHE supports addition + multiplication |
| Reversible? | **No** — hiding is one-way (no secret key to decrypt) | Yes — holder of private key can decrypt |
| Use case | ZKP, confidential transactions | Privacy-preserving cloud compute |
| Example | Bitcoin Confidential Transactions, rollup proofs | MIT's Enigma, FHE schemes |

Analogy:
```
Pedersen:  commit(a) * commit(b) = commit(a + b)
FHE:       enc(a) * enc(b)       = enc(a + b)  or  enc(a * b)
```

**Quick way to remember**: Pedersen is like a weaker, one-way cousin of FHE — only supports addition, and nobody can decrypt (that's a feature, not a bug, for commitments).

## What the Verifier Does

1. Receives: `C_a, C_b, C_c, S, r_total`
2. Computes: `lhs = (C_a * C_b * C_c) % p`
3. Computes: `rhs = commit(S, r_total)`
4. Checks: `lhs == rhs`

No Sigma protocol needed — the homomorphic property does all the work.

## Why It's Zero-Knowledge

- Verifier sees `C_a, C_b, C_c` but can't extract `a, b, c` (hiding property of Pedersen commitment)
- Verifier sees `r_total` but can't derive individual `r_a, r_b, r_c` (infinite combinations sum to `r_total`)
- Verifier only learns: the sum is `S`. Nothing about individual values.

## Edge Cases and Failure Modes

### 1. Wrong blinding factor
If prover provides incorrect `r_total` (e.g., `r_a + r_b` instead of `r_a + r_b + r_c`), the check fails.

### 2. Prover lies about the sum
If prover claims `S = 7` but actual sum is `6`, then `commit(7, r_total) != C_a * C_b * C_c`. The binding property prevents this.

### 3. Overflow in modular arithmetic
If `a + b + c >= p`, the sum wraps around mod p. For example with `p = 223`: `a=100, b=100, c=100` → sum = 300, but `300 mod 223 = 77`. The commitment math still works, but the "sum" is 77 not 300. This is fine in the math — but the application must be aware that arithmetic is mod p.

### 4. r_total leaks information?
No. Knowing `r_total = r_a + r_b + r_c` doesn't help recover individual `r` values. It's like knowing `x + y + z = 10` — infinite solutions.

### 5. What if g or h are not proper generators?
If `h = g^k` for some known `k`, the binding property breaks — prover could find different `(v', r')` that produces the same commitment. The security assumption is that nobody knows `log_g(h)`.

## Comparison with Ex1

| | Ex1 (Preimage) | Ex2 (Sum) |
|---|---|---|
| Proof technique | Sigma protocol + Fiat-Shamir | Homomorphic property only |
| Interactive? | Yes (then made non-interactive) | No — purely algebraic check |
| Complexity | 3-move protocol | Single equation |
| What's proven | "I know the secret" | "The sum is correct" |
