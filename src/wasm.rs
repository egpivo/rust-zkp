use crate::commitment::commit;
use crate::sigma::{Proof, challenge_for_tx, prove_commit, prove_response};
use crate::transaction::Transaction;
use num_bigint::BigUint;
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
