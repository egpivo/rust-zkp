use num_bigint::{BigUint, RandBigInt};
use sha2::{Sha256, Digest};

pub fn commit(v: &BigUint, r: &BigUint, g: &BigUint, h: &BigUint, p: &BigUint) -> BigUint {
    (g.modpow(v, p) * h.modpow(r, p)) % (p)
}

fn prove_commit(g: &BigUint, p: &BigUint) -> (BigUint, BigUint) {
    let mut rng = rand::thread_rng();
    let k = rng.gen_biguint_below(p);
    let r = g.modpow(&k, p);
    (k, r)
}

fn prove_response(k: &BigUint, e: &BigUint, secrect: &BigUint) -> BigUint {
    k + e * secrect
}


pub struct Proof {
    pub r: BigUint, // randomly commit g^k mod p 
    pub z: BigUint, // response = k + e * secrect
}



impl Proof {
    pub fn verify(proof: &Proof, c: &BigUint, e: &BigUint, g: &BigUint, p: &BigUint) -> bool {
        g.modpow(&proof.z, p) == (&proof.r * c.modpow(e, p)) % p
    }
}

fn challenge(g: &BigUint, c: &BigUint, r: &BigUint, p: &BigUint) -> BigUint {
    let mut hasher = Sha256::new();
    hasher.update(g.to_bytes_be());
    hasher.update(c.to_bytes_be());
    hasher.update(r.to_bytes_be());
    let hash = hasher.finalize();
    BigUint::from_bytes_be(&hash) % p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commit() {
        let p = BigUint::from(23u32);
        let g = BigUint::from(4u32);
        let h = BigUint::from(9u32);

        assert_eq!( 
            commit(&BigUint::from(2u32), &BigUint::from(3u32), &g, &h, &p),
            BigUint::from(3u32)
        );
        assert_eq!( 
            commit(&BigUint::from(2u32), &BigUint::from(4u32), &g, &h, &p),
            BigUint::from(4u32)
        );
    }

    #[test]
    fn test_sigma_protocol() {
        let p = BigUint::from(223u32);
        let g = BigUint::from(4u32);
        let secrect = BigUint::from(123u32);
        let c = g.modpow(&secrect, &p);

        // Step 1: prover generates commit
        let (k, r) = prove_commit(&g, &p);
        // Step 2: verifier obtains challenge
        let mut rng = rand::thread_rng();
        let e = rng.gen_biguint_below(&p);
        // Step 3: provers computes response
        let z = prove_response(&k, &e, &secrect);
        // Step 4: verification
        let proof = Proof {r, z};
        assert!(Proof::verify(&proof, &c, &e, &g, &p));
    }

    #[test]
    fn test_fiat_shamir() {
        let p = BigUint::from(223u32);
        let g = BigUint::from(4u32);
        let secrect = BigUint::from(123u32);
        let c = g.modpow(&secrect, &p);

        // Step 1: prover generates commit
        let (k, r) = prove_commit(&g, &p);
        // Step 2: verifier obtains challenge
        let e = challenge(&g, &c, &r, &p);
        // Step 3: provers computes response
        let z = prove_response(&k, &e, &secrect);
        // Step 4: verification
        let proof = Proof {r, z};
        assert!(Proof::verify(&proof, &c, &e, &g, &p));
    }

}