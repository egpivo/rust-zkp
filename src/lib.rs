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
    (0..num_bits)
        .map(|i| 
            if n.bit(i as u64) { BigUint::from(1u32) } 
            else { BigUint::from(0u32) }
        )
        .collect()
}

fn hash_pair(left: &BigUint, right: &BigUint) -> BigUint {
    let mut hasher = Sha256::new();
    hasher.update(left.to_bytes_be());
    hasher.update(right.to_bytes_be());
    BigUint::from_bytes_be(&hasher.finalize())
}

fn build_tree(leaves: Vec<BigUint>) -> BigUint {
    let mut level = leaves;
    while level.len() > 1 {
        let mut next_level = vec![];
        for pair in level.chunks(2) {
            next_level.push(hash_pair(&pair[0], &pair[1]));
        }
        level = next_level;
    }
    // or level.into_iter().next().unwrap()
    level.remove(0)
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
        let expected: Vec<BigUint> = 
            vec![1u32, 0, 1, 1].into_iter().map(BigUint::from)
            .collect();
        assert_eq!(
            to_bits(&BigUint::from(13u32), 4), 
            expected
        );
    }

    #[test]
    fn test_threshold() {
        let p = BigUint::from(223u32);
        let g = BigUint::from(4u32);
        let h = BigUint::from(3u32);
        let mut rng = rand::thread_rng();
        let r_v = rng.gen_biguint_below(&p);

        let v = BigUint::from(10u32);
        let t = BigUint::from(7u32);
        let delta = &v - &t;

        let c_v = commit(&v, &r_v, &g, &h, &p);
        let c_td = commit(&(&t + &delta), &r_v, &g, &h, &p);
        assert_eq!(c_v, c_td);

        // decompose delta into bits
        let bits = to_bits(&delta, 8);
        // reconstruct delta from bits: b_0*1 + b_1*2 + b_2*4 + ...
        let mut reconstructed = BigUint::from(0u32);
        for i in 0..bits.len() {
            reconstructed += &bits[i] * BigUint::from(1u32 << i);
        }
        assert_eq!(delta, reconstructed);
    }

    #[test]
    fn test_homomorphic() {
        let p = BigUint::from(223u32);
        let g = BigUint::from(4u32);
        let h = BigUint::from(3u32);
        let mut rng = rand::thread_rng();
        let r_v = rng.gen_biguint_below(&p);

        let v = BigUint::from(10u32);
        let t = BigUint::from(7u32);
        let delta = &v - &t;
    
        // Prover
        let bits = to_bits(&delta, 8);
        let r_bits: Vec<BigUint> = (0..8).map(|_| rng.gen_biguint_below(&p)).collect();
        let c_bits: Vec<BigUint> = bits.iter().zip(&r_bits)
            .map(|(b, r)| commit(b, r, &g, &h, &p))
            .collect();
        
        // r_delta == sum of r_i * 2^i
        let r_delta: BigUint = (0..8).map(|i| &r_bits[i] * BigUint::from(1u32 << i)).sum();

        // Verifier: compute product of C_i^(2^i)
        let mut lhs = BigUint::from(1u32);
        for i in 0..8 {
            lhs = (lhs * c_bits[i].modpow(&BigUint::from(1u32 << i), &p)) % &p;
        }
        let rhs = commit(&delta, &r_delta, &g, &h, &p);
        assert_eq!(lhs, rhs);
    }

    #[test]
    fn test_hash_pair() {
        let a = BigUint::from(3u32);
        let b = BigUint::from(4u32);
        let h1 = hash_pair(&a, &b);
        let h2 = hash_pair(&a, &b);
        assert_eq!(h1, h2);

        let h3 = hash_pair(&b, &a);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_build_tree() {
        let v0 = BigUint::from(3u32);
        let v1 = BigUint::from(4u32);
        let v2 = BigUint::from(5u32);
        let v3 = BigUint::from(6u32);

        let h01 = hash_pair(&v0, &v1);
        let h23 = hash_pair(&v2, &v3); 
        let expected_root = hash_pair(&h01, &h23);
        
        let leaves = vec![v0, v1, v2, v3];
        let root = build_tree(leaves);

        assert_eq!(root, expected_root);
    }
}