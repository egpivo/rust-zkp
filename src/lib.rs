pub mod commitment;
pub mod sigma;
pub mod bits;
pub mod merkle;


#[cfg(test)]
mod tests {
    use num_bigint::{BigUint, RandBigInt};
    use crate::commitment::commit;
    use crate::sigma::{prove_commit, prove_response, challenge, Proof};
    use crate::bits::to_bits;

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

}