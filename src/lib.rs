pub mod account;
pub mod batch;
pub mod bits;
pub mod commitment;
pub mod dto;
pub mod merkle;
pub mod sigma;
pub mod transaction;

#[cfg(feature = "server")]
pub mod error;
#[cfg(feature = "server")]
pub mod state;
#[cfg(feature = "server")]
pub mod storage;

#[cfg(feature = "wasm")]
pub mod wasm;

#[cfg(test)]
mod tests {
    use crate::sigma::{Proof, challenge, prove_commit, prove_response};
    use num_bigint::BigUint;

    #[test]
    fn test_fiat_shamir() {
        let p = BigUint::from(223u32);
        let g = BigUint::from(4u32);
        let secret = BigUint::from(123u32);
        let c = g.modpow(&secret, &p);

        // Step 1: prover generates commit
        let (k, r) = prove_commit(&g, &p);
        // Step 2: verifier obtains challenge
        let e = challenge(&g, &c, &r, &p);
        // Step 3: prover computes response
        let z = prove_response(&k, &e, &secret);
        // Step 4: verification
        let proof = Proof { r, z };
        assert!(Proof::verify(&proof, &c, &e, &g, &p));
    }
}
