use num_bigint::{BigUint, RandBigInt};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub fn prove_commit(g: &BigUint, p: &BigUint) -> (BigUint, BigUint) {
    let mut rng = rand::thread_rng();
    let k = rng.gen_biguint_below(p);
    let r = g.modpow(&k, p);
    (k, r)
}

pub fn prove_response(k: &BigUint, e: &BigUint, secret: &BigUint) -> BigUint {
    k + e * secret
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proof {
    pub r: BigUint, // randomly commit g^k mod p
    pub z: BigUint, // response = k + e * secret
}

impl Proof {
    pub fn verify(proof: &Proof, c: &BigUint, e: &BigUint, g: &BigUint, p: &BigUint) -> bool {
        g.modpow(&proof.z, p) == (&proof.r * c.modpow(e, p)) % p
    }
}

pub fn challenge(g: &BigUint, c: &BigUint, r: &BigUint, p: &BigUint) -> BigUint {
    let mut hasher = Sha256::new();
    hasher.update(g.to_bytes_be());
    hasher.update(c.to_bytes_be());
    hasher.update(r.to_bytes_be());
    let hash = hasher.finalize();
    BigUint::from_bytes_be(&hash) % p
}

pub fn challenge_for_tx(
    g: &BigUint,
    pubkey: &BigUint,
    r: &BigUint,
    p: &BigUint,
    message: &[u8],
) -> BigUint {
    let mut hasher = Sha256::new();
    hasher.update(g.to_bytes_be());
    hasher.update(pubkey.to_bytes_be());
    hasher.update(r.to_bytes_be());
    hasher.update(message);
    BigUint::from_bytes_be(&hasher.finalize()) % p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sigma_protocol() {
        let p = BigUint::from(223u32);
        let g = BigUint::from(4u32);
        let secret = BigUint::from(123u32);
        let c = g.modpow(&secret, &p);

        // Step 1: prover generates commit
        let (k, r) = prove_commit(&g, &p);
        // Step 2: verifier obtains challenge
        let mut rng = rand::thread_rng();
        let e = rng.gen_biguint_below(&p);
        // Step 3: prover computes response
        let z = prove_response(&k, &e, &secret);
        // Step 4: verification
        let proof = Proof { r, z };
        assert!(Proof::verify(&proof, &c, &e, &g, &p));
    }
}
