use num_bigint::BigUint;

pub fn commit(v: &BigUint, r: &BigUint, g: &BigUint, h: &BigUint, p: &BigUint) -> BigUint {
    (g.modpow(v, p) * h.modpow(r, p)) % (p)
}


#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::RandBigInt;    
    use crate::commitment::commit;

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
    fn test_prov_sum() {
        let p = BigUint::from(223u32).pow(127);
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

}