---
layout: ../../layouts/Layout.astro
title: "ZKP in Blockchain: ZK-Rollup"
---

# ZKP in Blockchain: ZK-Rollup

## The Core Problem

Executing transactions on-chain is expensive (gas). But skipping verification is unsafe. ZKP solves this: **verify cheaply without re-executing**.

## How ZK-Rollup Works

```
Off-chain (Prover = rollup operator)        On-chain (Verifier = smart contract)
  |                                            |
  |  1. Execute 1000s of txns off-chain        |
  |     (a sends 10 to b, c sends 5 to d...)   |
  |                                            |
  |  2. Compute proof:                         |
  |     "all txns are valid,                   |
  |      final state is S"                     |
  |                                            |
  |  3. Submit proof + new state S             |
  |  --------------- tx ------------------>    |
  |                                            |
  |                          4. verify(proof, S)
  |                             (very cheap)   |
  |                                            |
  |                          5. pass → accept S|
  |                             fail → reject  |
```

## Why Non-Interactive (Fiat-Shamir)

A smart contract cannot "send a challenge and wait for a reply" — it can only passively receive a transaction and verify. So the proof must be self-contained: Fiat-Shamir.

## Why It Saves Gas

| Approach | Cost |
|----------|------|
| Execute all txns on-chain | O(n) gas — each txn costs gas |
| Trust the operator blindly | 0 gas — but not secure |
| ZK-Rollup | O(1) gas — verify one proof regardless of how many txns |

The proof size and verification cost are roughly constant, no matter if you bundled 100 or 10,000 transactions. This is the ultimate gas optimization.

## The Tradeoff

- **Proving is expensive**: the off-chain prover needs significant compute to generate the proof
- **Verification is cheap**: the on-chain contract only runs a small check
- This is a good tradeoff because: compute is cheap off-chain, but gas is expensive on-chain

## Common Misconceptions (Q&A)

**Q: Is the on-chain state the operator's account?**
No. The state is a single Merkle root stored in the smart contract. It represents a snapshot of all accounts inside the rollup. The contract doesn't know individual account details — just the 32-byte root hash.

**Q: Who owns the smart contract?**
Nobody. A smart contract is code deployed on-chain. Once deployed, the logic is immutable. Anyone can read it, anyone can call `verify`. The operator has no special privilege over the contract — they just have the compute power to generate proofs.

**Q: Can the operator cheat?**
No. Unlike a centralized exchange where you trust the operator, ZK-Rollup forces the operator to submit a valid proof. The math locks them out of cheating. The operator is just a "worker" who executes transactions and computes proofs.

**Q: Does anyone on-chain know who the users are?**
No. The chain only sees a Merkle root update. It doesn't know which users transacted, how much they sent, or to whom. Users can independently verify correctness through the proof, but their identities stay private.

**Q: Is a rollup like a private ledger for the operator?**
Not exactly. It's more like a **public, tamper-proof lockbox**. The operator processes transactions inside it, but the lock is mathematical — the operator can't steal or forge anything, and anyone can verify the lock hasn't been broken.

**Q: Can anyone deploy a smart contract?**
Yes. But "no ownership" depends on **how the code is written**. A well-designed contract has no `owner` or admin privileges — pure math verification. A poorly designed one could have backdoors. This is why open-source code and audits matter in DeFi.

## Real-World Examples

- **zkSync** — general-purpose ZK-Rollup on Ethereum
- **StarkNet** — uses STARKs (no trusted setup)
- **Polygon zkEVM** — EVM-compatible ZK-Rollup
