pub mod commitment;
pub mod sigma;
pub mod bits;
pub mod merkle;
pub mod account;
pub mod state;


#[cfg(test)]
mod tests {
    use num_bigint::BigUint;
    use crate::sigma::{prove_commit, prove_response, challenge, Proof};

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
        let proof = Proof {r, z};
        assert!(Proof::verify(&proof, &c, &e, &g, &p));
    }

}