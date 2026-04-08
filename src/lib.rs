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

fn prove_response(k: &BigUint, e: &BigUint, secret: &BigUint) -> BigUint {
    k + e * secret
}


pub struct Proof {
    pub r: BigUint, // randomly commit g^k mod p 
    pub z: BigUint, // response = k + e * secret
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

fn to_bits(n: &BigUint, num_bits: usize) -> Vec<BigUint> {
    let mut vecs = vec![BigUint::from(0u32); num_bits];
    let mut a = n.clone();
    let mut i = 0;
    while a > BigUint::from(0u32) && i < num_bits {
        vecs[i] = &a %  BigUint::from(2u32);
        a /= BigUint::from(2u32);
        i += 1;
    }
    vecs
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
        let proof = Proof {r, z};
        assert!(Proof::verify(&proof, &c, &e, &g, &p));
    }

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


    #[test]
    fn test_prov_sum() {
        let p = BigUint::from(223u32);
        let g = BigUint::from(4u32);
        let h = BigUint::from(9u32);
        let mut rng = rand::thread_rng();

        let a = BigUint::from(1u32);
        let b = BigUint::from(2u32);
        let c = BigUint::from(3u32);
        let r_a = rng.gen_biguint_below(&p);
        let r_b = rng.gen_biguint_below(&p);
        let r_c = rng.gen_biguint_below(&p);

        // Prover: commit each value
        let c_a = commit(&a, &r_a, &g, &h, &p);
        let c_b = commit(&b, &r_b, &g, &h, &p);
        let c_c = commit(&c, &r_c, &g, &h, &p);        
  
        // Case I: correct sum
        // Prover: publish S and r_total
        let s = &a + &b + &c;
        let r_total = &r_a + &r_b + &r_c;
        
        // Verifier: check homomorphic property
        let lhs = (&c_a * &c_b * &c_c) % &p;
        let rhs = commit(&s, &r_total, &g, &h, &p);
        assert_eq!(lhs, rhs);

        // Case II: wrong sum
        let wrong_s = BigUint::from(1235u32);
        let wrong_sum_rhs = commit(&wrong_s, &r_total, &g, &h, &p);
        assert_ne!(lhs, wrong_sum_rhs);

        // Case III: wrong r_total
        let wrong_r_total = &r_a + &r_b;
        let wrong_rtotal_rhs = commit(&s, &wrong_r_total, &g, &h, &p);
        assert_ne!(lhs, wrong_rtotal_rhs);
    }

    #[test]
    fn test_to_bits() {
        assert_eq!(
            to_bits(&BigUint::from(13u32), 4), 
            vec![BigUint::from(1u32), BigUint::from(0u32), BigUint::from(1u32), BigUint::from(1u32)]);
    }

}