use num_bigint::BigUint;

pub fn commit(v: &BigUint, r: &BigUint, g: &BigUint, h: &BigUint, p: &BigUint) -> BigUint {
    (g.modpow(v, p) * h.modpow(r, p)) % (p)
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
}