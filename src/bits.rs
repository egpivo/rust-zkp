use num_bigint::BigUint;

pub fn to_bits(n: &BigUint, num_bits: usize) -> Vec<BigUint> {
    (0..num_bits)
        .map(|i| 
            if n.bit(i as u64) { BigUint::from(1u32) } 
            else { BigUint::from(0u32) }
        )
        .collect()
}



#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::RandBigInt;
    use crate::commitment::commit;

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

}