# Ex4: Prove Membership (Merkle Tree)

## ZKP Four Elements

| Element | Content |
|---------|---------|
| **Witness** | the leaf `v`, its `index`, and the Merkle path (sibling hashes) |
| **Public Input** | Merkle root `R` |
| **Relation** | `verify_merkle(v, index, path, R) == true` |
| **What Verifier Checks** | the leaf + path reconstructs to the same root, without knowing all other set members |

## Merkle Tree Structure

```
            root
           /    \
         h01    h23
        /  \   /  \
       v0  v1 v2  v3   ← leaves (the set)
```

Where:
- `h01 = hash(v0 || v1)`
- `h23 = hash(v2 || v3)`
- `root = hash(h01 || h23)`

## Membership Proof (the "path")

To prove `v2 ∈ set`, the prover gives the verifier:
- `v2` (the leaf)
- `index = 2`
- `path = [v3, h01]` (the sibling at each level)

Then the verifier reconstructs:
```
computed = hash_pair(v2, v3)        // index=2 even → leaf is left, sibling is right
computed = hash_pair(h01, computed) // index=1 odd  → sibling is left, computed is right
check: computed == root ?
```

## Q&A — Common Confusions

### What is `index`?

It's the position of the leaf in the original `leaves` array.

```
leaves = [v0, v1, v2, v3]
          ↑   ↑   ↑   ↑
       idx=0 1   2   3
```

The index serves two purposes:
1. **Find the sibling**: even index → sibling at `index+1`; odd index → sibling at `index-1`
2. **Determine hash order**: even index → leaf is on the left; odd index → leaf is on the right

Each level up, `index /= 2` (two children merge into one parent).

### Why doesn't the proof include the root?

Because **the root is the public input**. The verifier already has it (e.g., stored on-chain).

If the prover supplied the root, the verifier would ask: "How do I know your root is the real one?" — defeating the purpose. The verifier reconstructs a root from the leaf + path, then compares against their own trusted copy.

So `proof.len() == log2(n)`. For 4 leaves: 2 sibling hashes. For 1024 leaves: only 10 sibling hashes. **This is the magic of Merkle trees — proof size is logarithmic.**

### Why doesn't the leaf get "isolated" when we extract the sibling?

Looking at this loop:
```rust
for pair in level.chunks(2) {
    next_level.push(hash_pair(&pair[0], &pair[1]));
}
```

`chunks(2)` pairs up adjacent elements regardless. The sibling is **also still part of the pair** when we hash up to the next level. Extracting it for the proof is just *recording* it — it still participates in building the tree.

So `merkle_proof` does two things in each iteration:
1. **Record** the sibling (push to `proof`)
2. **Build** the next level normally (same as `build_tree`)

## Why It's Zero-Knowledge (sort of)

Strictly, a basic Merkle proof is **NOT zero-knowledge** — it reveals which leaf you're proving membership of (anyone can see `v` and `index`).

To make it ZK, you'd typically:
- Commit to `v` (Pedersen) and prove "the committed value is in the set"
- Or use a polynomial commitment / vector commitment scheme

In our exercise, we focus on the basic Merkle proof. The "ZK-flavored" upgrade is a separate layer.

## Use Cases in Real World

- **Bitcoin SPV**: light clients verify a transaction is in a block via Merkle proof, without downloading the full block
- **ZK-Rollup state**: each account is a leaf in a giant Merkle tree; the rollup contract only stores the root
- **Whitelisting**: prove your address is in a set (e.g., airdrop, allowlist) without revealing which one — combined with ZK
- **Certificate Transparency**: log of issued TLS certs uses Merkle trees

## Rust Lessons

### `Vec<T>::chunks(2)`
Creates an iterator over slices of size 2 (the last chunk may be smaller). Useful for pair-wise processing.

### `level.remove(0)` vs `level.into_iter().next().unwrap()` vs `level[0].clone()`
All extract the first element from a `Vec`:
- `remove(0)` — moves it out, shifts the rest (O(n) but cheap when length=1)
- `into_iter().next().unwrap()` — consumes the vec, no shifting
- `[0].clone()` — keeps the vec intact, but extra clone

For length=1 vectors, all are equivalent in practice.

### `to_vec()` for borrowing
```rust
fn merkle_proof(leaves: &[BigUint], mut index: usize) -> Vec<BigUint> {
    let mut level: Vec<BigUint> = leaves.to_vec();
    ...
}
```
We take `&[BigUint]` (a borrow) but need an owned `Vec` to mutate. `to_vec()` clones the slice into a new `Vec`.

### `mut` parameter
```rust
fn merkle_proof(leaves: &[BigUint], mut index: usize) -> Vec<BigUint>
```
The `mut` on `index` allows us to reassign `index = index / 2` inside the function. Doesn't affect the caller — only the local copy.
