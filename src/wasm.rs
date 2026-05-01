use crate::bits::to_bits;
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
