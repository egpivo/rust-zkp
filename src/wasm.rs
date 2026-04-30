use wasm_bindgen::prelude::*;
use num_bigint::BigUint;
use crate::commitment::commit;

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

