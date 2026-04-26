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
}