use crate::bits::{BitOrProof, prove_bit_or, to_bits, verify_bit_or};
use crate::commitment::commit;
use crate::merkle::{build_tree, hash_pair, merkle_proof, verify_merkle};
use crate::sigma::{Proof, challenge_for_tx, prove_commit, prove_response};
use crate::transaction::Transaction;
use num_bigint::{BigUint, RandBigInt};
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// Compute Pedersen commitment: g^v * h^r mod p
#[wasm_bindgen]
pub fn pedersen_commit(v: &str, r: &str, g: &str, h: &str, p: &str) -> String {
    let v = BigUint::parse_bytes(v.as_bytes(), 10).unwrap();
    let r = BigUint::parse_bytes(r.as_bytes(), 10).unwrap();
    let g = BigUint::parse_bytes(g.as_bytes(), 10).unwrap();
    let h = BigUint::parse_bytes(h.as_bytes(), 10).unwrap();
    let p = BigUint::parse_bytes(p.as_bytes(), 10).unwrap();
    commit(&v, &r, &g, &h, &p).to_string()
}

#[wasm_bindgen]
pub fn sign_transaction(
    from: u32,
    to: u32,
    amount: u64,
    nonce: u64,
    secret_str: &str,
    p_str: &str,
    g_str: &str,
) -> String {
    let secret = BigUint::parse_bytes(secret_str.as_bytes(), 10).unwrap();
    let p = BigUint::parse_bytes(p_str.as_bytes(), 10).unwrap();
    let g = BigUint::parse_bytes(g_str.as_bytes(), 10).unwrap();

    let pubkey = g.modpow(&secret, &p);

    // build message
    let mut msg = vec![];
    msg.extend(from.to_be_bytes());
    msg.extend(to.to_be_bytes());
    msg.extend(amount.to_be_bytes());
    msg.extend(nonce.to_be_bytes());

    // sign
    let (k, r) = prove_commit(&g, &p);
    let e = challenge_for_tx(&g, &pubkey, &r, &p, &msg);
    let z = prove_response(&k, &e, &secret);

    // contruct tx
    let tx = Transaction {
        from,
        to,
        amount,
        nonce,
        proof: Proof { r, z },
        challenge_e: e,
    };

    serde_json::to_string(&tx).unwrap()
}

// ------------------ Sigma Protocol step-by-step ------------------

#[derive(Serialize)]
struct SigmaCommit {
    k: String,
    r: String,
    pubkey: String,
}

#[wasm_bindgen]
pub fn sigma_pubkey(secret_str: &str, g_str: &str, p_str: &str) -> String {
    let secret = BigUint::parse_bytes(secret_str.as_bytes(), 10).unwrap();
    let g = BigUint::parse_bytes(g_str.as_bytes(), 10).unwrap();
    let p = BigUint::parse_bytes(p_str.as_bytes(), 10).unwrap();
    g.modpow(&secret, &p).to_string()
}

/// Step 1: prover generates random k and commitment r = g^k mod p
#[wasm_bindgen]
pub fn sigma_commit(secret_str: &str, g_str: &str, p_str: &str) -> String {
    let secret = BigUint::parse_bytes(secret_str.as_bytes(), 10).unwrap();
    let g = BigUint::parse_bytes(g_str.as_bytes(), 10).unwrap();
    let p = BigUint::parse_bytes(p_str.as_bytes(), 10).unwrap();
    let pubkey = g.modpow(&secret, &p);
    let (k, r) = prove_commit(&g, &p);
    serde_json::to_string(&SigmaCommit {
        k: k.to_string(),
        r: r.to_string(),
        pubkey: pubkey.to_string(),
    })
    .unwrap()
}

/// Step 2: verifier picks a random challenge e in [0, p)
#[wasm_bindgen]
pub fn sigma_random_challenge(p_str: &str) -> String {
    let p = BigUint::parse_bytes(p_str.as_bytes(), 10).unwrap();
    let mut rng = rand::thread_rng();
    rng.gen_biguint_below(&p).to_string()
}

/// Step 3: prover computes z = k + e * secret
#[wasm_bindgen]
pub fn sigma_response(k_str: &str, e_str: &str, secret_str: &str) -> String {
    let k = BigUint::parse_bytes(k_str.as_bytes(), 10).unwrap();
    let e = BigUint::parse_bytes(e_str.as_bytes(), 10).unwrap();
    let secret = BigUint::parse_bytes(secret_str.as_bytes(), 10).unwrap();
    prove_response(&k, &e, &secret).to_string()
}

#[derive(Serialize)]
struct SigmaVerifyResult {
    lhs: String,
    rhs: String,
    valid: bool,
}

/// Step 4: verifier checks g^z mod p == r * c^e mod p
#[wasm_bindgen]
pub fn sigma_verify_explain(
    r_str: &str,
    z_str: &str,
    e_str: &str,
    c_str: &str,
    g_str: &str,
    p_str: &str,
) -> String {
    let r = BigUint::parse_bytes(r_str.as_bytes(), 10).unwrap();
    let z = BigUint::parse_bytes(z_str.as_bytes(), 10).unwrap();
    let e = BigUint::parse_bytes(e_str.as_bytes(), 10).unwrap();
    let c = BigUint::parse_bytes(c_str.as_bytes(), 10).unwrap();
    let g = BigUint::parse_bytes(g_str.as_bytes(), 10).unwrap();
    let p = BigUint::parse_bytes(p_str.as_bytes(), 10).unwrap();

    let lhs = g.modpow(&z, &p);
    let rhs = (&r * c.modpow(&e, &p)) % &p;
    let valid = lhs == rhs;
    serde_json::to_string(&SigmaVerifyResult {
        lhs: lhs.to_string(),
        rhs: rhs.to_string(),
        valid,
    })
    .unwrap()
}

// ------------------ Merkle ------------------

fn parse_leaves(leaves_csv: &str) -> Vec<BigUint> {
    leaves_csv
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| BigUint::parse_bytes(s.as_bytes(), 10).expect("bad leaf"))
        .collect()
}

fn pad_to_pow2(mut leaves: Vec<BigUint>) -> Vec<BigUint> {
    let target = leaves.len().next_power_of_two().max(1);
    while leaves.len() < target {
        leaves.push(BigUint::from(0u32));
    }
    leaves
}

#[wasm_bindgen]
pub fn merkle_root(leaves_csv: &str) -> String {
    let leaves = pad_to_pow2(parse_leaves(leaves_csv));
    if leaves.is_empty() {
        return "0".to_string();
    }
    build_tree(leaves).to_string()
}

#[derive(Serialize)]
struct MerkleProofResult {
    leaf: String,
    index: usize,
    path: Vec<String>,
    root: String,
}

#[wasm_bindgen]
pub fn merkle_path(leaves_csv: &str, index: usize) -> String {
    let leaves = pad_to_pow2(parse_leaves(leaves_csv));
    let leaf = leaves[index].clone();
    let proof = merkle_proof(&leaves, index);
    let root = build_tree(leaves);
    serde_json::to_string(&MerkleProofResult {
        leaf: leaf.to_string(),
        index,
        path: proof.iter().map(|h| h.to_string()).collect(),
        root: root.to_string(),
    })
    .unwrap()
}

#[derive(Serialize)]
struct MerkleVerifyStep {
    role: String, // "leaf" / "left" / "right"
    left: String,
    right: String,
    out: String,
}

#[wasm_bindgen]
pub fn merkle_verify_explain(
    leaf_str: &str,
    index: usize,
    path_csv: &str,
    root_str: &str,
) -> String {
    let leaf = BigUint::parse_bytes(leaf_str.as_bytes(), 10).unwrap();
    let path: Vec<BigUint> = path_csv
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| BigUint::parse_bytes(s.as_bytes(), 10).unwrap())
        .collect();
    let root = BigUint::parse_bytes(root_str.as_bytes(), 10).unwrap();

    let mut steps = Vec::new();
    let mut idx = index;
    let mut computed = leaf.clone();

    for sibling in &path {
        let (left, right) = if idx.is_multiple_of(2) {
            (computed.clone(), sibling.clone())
        } else {
            (sibling.clone(), computed.clone())
        };
        let out = hash_pair(&left, &right);
        steps.push(MerkleVerifyStep {
            role: if idx.is_multiple_of(2) {
                "leaf-left".into()
            } else {
                "leaf-right".into()
            },
            left: left.to_string(),
            right: right.to_string(),
            out: out.to_string(),
        });
        computed = out;
        idx /= 2;
    }

    let valid = computed == root;
    let result = serde_json::json!({
        "steps": steps,
        "computed_root": computed.to_string(),
        "expected_root": root.to_string(),
        "valid": valid,
    });
    serde_json::to_string(&result).unwrap()
}

#[wasm_bindgen]
pub fn merkle_verify_simple(leaf_str: &str, index: usize, path_csv: &str, root_str: &str) -> bool {
    let leaf = BigUint::parse_bytes(leaf_str.as_bytes(), 10).unwrap();
    let path: Vec<BigUint> = path_csv
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| BigUint::parse_bytes(s.as_bytes(), 10).unwrap())
        .collect();
    let root = BigUint::parse_bytes(root_str.as_bytes(), 10).unwrap();
    verify_merkle(&leaf, index, &path, &root)
}

// ------------------ Range proof (bit decomposition) ------------------

#[derive(Serialize)]
struct BitsResult {
    delta: String,
    bits: Vec<String>,
    reconstructed: String,
}

#[wasm_bindgen]
pub fn range_decompose(v_str: &str, t_str: &str, num_bits: usize) -> String {
    let v = BigUint::parse_bytes(v_str.as_bytes(), 10).unwrap();
    let t = BigUint::parse_bytes(t_str.as_bytes(), 10).unwrap();

    if v < t {
        let result = serde_json::json!({
            "error": "v < T: cannot prove non-negative delta"
        });
        return serde_json::to_string(&result).unwrap();
    }

    let delta = &v - &t;
    let bits = to_bits(&delta, num_bits);
    let mut reconstructed = BigUint::from(0u32);
    for (i, b) in bits.iter().enumerate() {
        reconstructed += b * BigUint::from(1u32 << i);
    }

    serde_json::to_string(&BitsResult {
        delta: delta.to_string(),
        bits: bits.iter().map(|b| b.to_string()).collect(),
        reconstructed: reconstructed.to_string(),
    })
    .unwrap()
}

// ------------------ Bit-OR (Pedersen bit ∈ {0,1}) ------------------

fn bit_or_proof_json(p: &BitOrProof) -> serde_json::Value {
    serde_json::json!({
        "a0": p.a0.to_string(),
        "a1": p.a1.to_string(),
        "s0": p.s0.to_string(),
        "s1": p.s1.to_string(),
        "e_fake": p.e_fake.to_string(),
        "fake_is_branch1": p.fake_is_branch1,
    })
}

fn parse_proof_json(proof_json: &str) -> Result<BitOrProof, String> {
    let v: serde_json::Value = serde_json::from_str(proof_json).map_err(|e| e.to_string())?;
    let g = |k: &str| -> Result<BigUint, String> {
        v[k].as_str()
            .ok_or_else(|| format!("missing or non-string field: {k}"))?
            .parse::<BigUint>()
            .map_err(|e| format!("{k}: {e}"))
    };
    Ok(BitOrProof {
        a0: g("a0")?,
        a1: g("a1")?,
        s0: g("s0")?,
        s1: g("s1")?,
        e_fake: g("e_fake")?,
        fake_is_branch1: v["fake_is_branch1"]
            .as_bool()
            .ok_or_else(|| "missing fake_is_branch1".to_string())?,
    })
}

/// Build `C = g^b h^r` and a Schnorr-OR proof that `b ∈ {0,1}` (Fiat–Shamir). Returns JSON.
#[wasm_bindgen]
pub fn bit_or_prove(b_str: &str, r_str: &str, g_str: &str, h_str: &str, p_str: &str) -> String {
    let parse = |s: &str, name: &str| {
        s.parse::<BigUint>()
            .map_err(|_| format!("invalid {name}: must be decimal BigUint"))
    };
    let res = (|| {
        let b = parse(b_str, "b")?;
        if b != BigUint::from(0u32) && b != BigUint::from(1u32) {
            return Err("b must be 0 or 1".to_string());
        }
        let r = parse(r_str, "r")?;
        let g = parse(g_str, "g")?;
        let h = parse(h_str, "h")?;
        let p = parse(p_str, "p")?;
        let c = commit(&b, &r, &g, &h, &p);
        let proof = prove_bit_or(&b, &r, &c, &g, &h, &p);
        let verify_ok = verify_bit_or(&proof, &c, &g, &h, &p);
        Ok(serde_json::json!({
            "c_bit": c.to_string(),
            "proof": bit_or_proof_json(&proof),
            "verify_ok": verify_ok,
        }))
    })();
    match res {
        Ok(j) => j.to_string(),
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    }
}

/// Verify a `proof` JSON (same shape as `bit_or_prove` → `proof`) against `c_bit`.
#[wasm_bindgen]
pub fn bit_or_verify(
    c_bit_str: &str,
    proof_json: &str,
    g_str: &str,
    h_str: &str,
    p_str: &str,
) -> String {
    let res: Result<bool, String> = (|| {
        let c = c_bit_str
            .parse::<BigUint>()
            .map_err(|_| "invalid c_bit".to_string())?;
        let proof = parse_proof_json(proof_json)?;
        let g = g_str
            .parse::<BigUint>()
            .map_err(|_| "invalid g".to_string())?;
        let h = h_str
            .parse::<BigUint>()
            .map_err(|_| "invalid h".to_string())?;
        let p = p_str
            .parse::<BigUint>()
            .map_err(|_| "invalid p".to_string())?;
        Ok(verify_bit_or(&proof, &c, &g, &h, &p))
    })();
    match res {
        Ok(valid) => serde_json::json!({ "valid": valid }).to_string(),
        Err(e) => serde_json::json!({ "error": e, "valid": false }).to_string(),
    }
}

/// Ex3-style: bit decomposition + Pedersen commitments + bit-OR per bit + homomorphic check.
#[wasm_bindgen]
pub fn range_zk_verify(
    v_str: &str,
    t_str: &str,
    num_bits: usize,
    g_str: &str,
    h_str: &str,
    p_str: &str,
) -> String {
    const MAX_BITS: usize = 32;
    if num_bits == 0 || num_bits > MAX_BITS {
        return serde_json::json!({
            "error": format!("num_bits must be 1..={MAX_BITS}")
        })
        .to_string();
    }
    let res = (|| {
        let v = v_str
            .parse::<BigUint>()
            .map_err(|_| "invalid v".to_string())?;
        let t = t_str
            .parse::<BigUint>()
            .map_err(|_| "invalid T".to_string())?;
        let g = g_str
            .parse::<BigUint>()
            .map_err(|_| "invalid g".to_string())?;
        let h = h_str
            .parse::<BigUint>()
            .map_err(|_| "invalid h".to_string())?;
        let p = p_str
            .parse::<BigUint>()
            .map_err(|_| "invalid p".to_string())?;

        if v < t {
            return Err("v < T: cannot prove non-negative delta".to_string());
        }

        let delta = &v - &t;
        let bits = to_bits(&delta, num_bits);
        let mut rng = rand::thread_rng();
        let r_bits: Vec<BigUint> = (0..num_bits).map(|_| rng.gen_biguint_below(&p)).collect();
        let c_bits: Vec<BigUint> = bits
            .iter()
            .zip(&r_bits)
            .map(|(b, r)| commit(b, r, &g, &h, &p))
            .collect();

        let mut bit_rows = Vec::new();
        let mut all_or_ok = true;
        for (i, (b, (c_i, r_i))) in bits.iter().zip(c_bits.iter().zip(&r_bits)).enumerate() {
            let proof = prove_bit_or(b, r_i, c_i, &g, &h, &p);
            let ok = verify_bit_or(&proof, c_i, &g, &h, &p);
            all_or_ok &= ok;
            bit_rows.push(serde_json::json!({
                "index": i,
                "b": b.to_string(),
                "r": r_i.to_string(),
                "c": c_i.to_string(),
                "or_valid": ok,
                "proof": bit_or_proof_json(&proof),
            }));
        }

        let r_delta: BigUint = (0..num_bits)
            .map(|i| &r_bits[i] * BigUint::from(1u32 << i))
            .sum();
        let mut lhs = BigUint::from(1u32);
        for (i, c_i) in c_bits.iter().enumerate() {
            lhs = (lhs * c_i.modpow(&BigUint::from(1u32 << i), &p)) % &p;
        }
        let rhs = commit(&delta, &r_delta, &g, &h, &p);
        let homomorphic_ok = lhs == rhs;

        let mut reconstructed = BigUint::from(0u32);
        for (i, b) in bits.iter().enumerate() {
            reconstructed += b * BigUint::from(1u32 << i);
        }
        let bits_reconstruct_ok = reconstructed == delta;

        Ok(serde_json::json!({
            "delta": delta.to_string(),
            "bits": bits.iter().map(|b| b.to_string()).collect::<Vec<_>>(),
            "homomorphic_ok": homomorphic_ok,
            "bits_reconstruct_ok": bits_reconstruct_ok,
            "all_bit_or_ok": all_or_ok,
            "all_ok": all_or_ok && homomorphic_ok && bits_reconstruct_ok,
            "bit_rows": bit_rows,
        }))
    })();
    match res {
        Ok(j) => j.to_string(),
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    }
}
