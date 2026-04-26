use num_bigint::BigUint;
use sha2::{Sha256, Digest};

pub fn hash_pair(left: &BigUint, right: &BigUint) -> BigUint {
    let mut hasher = Sha256::new();
    hasher.update(left.to_bytes_be());
    hasher.update(right.to_bytes_be());
    BigUint::from_bytes_be(&hasher.finalize())
}

pub fn build_tree(leaves: Vec<BigUint>) -> BigUint {
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

pub fn merkle_proof(leaves: &[BigUint], mut index: usize) -> Vec<BigUint> {
    let mut proof = vec![];
    let mut level: Vec<BigUint> = leaves.to_vec();

    while level.len() > 1 {
        let sibling_index = if index % 2 == 0 { index + 1 } else { index - 1 };
        proof.push(level[sibling_index].clone());

        let mut next_level = vec![];
        for pair in level.chunks(2) {
            next_level.push(hash_pair(&pair[0], &pair[1]));
        }

        level = next_level;
        index /= 2;
    }
    proof
}

pub fn verify_merkle(leaf: &BigUint, mut index: usize, path: &[BigUint], root: &BigUint) -> bool {
    let mut computed = leaf.clone();

    for sibling in path {
        if index % 2 == 0 {
            computed = hash_pair(&computed, sibling);
        } else {
            computed = hash_pair(sibling, &computed);
        }
        index /= 2;
    }

    &computed == root

}




#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_merkle_proof() {
        let v0 = BigUint::from(3u32);
        let v1 = BigUint::from(4u32);
        let v2 = BigUint::from(5u32);
        let v3 = BigUint::from(6u32);
        let leaves = vec![v0.clone(), v1.clone(), v2.clone(), v3.clone()];

        let h01 = hash_pair(&v0, &v1);
        let proof = merkle_proof(&leaves, 2); 
        assert_eq!(proof.len(), 2);
        assert_eq!(proof[0], v3);
        assert_eq!(proof[1], h01);
    }    

    #[test]
    fn test_merkle_membership() {
        let v0 = BigUint::from(3u32);
        let v1 = BigUint::from(4u32);
        let v2 = BigUint::from(5u32);
        let v3 = BigUint::from(6u32);
        let leaves = vec![v0.clone(), v1.clone(), v2.clone(), v3.clone()];

        let root = build_tree(leaves.clone());
        let proof = merkle_proof(&leaves, 2);
        
        assert!(verify_merkle(&v2, 2, &proof, &root));

        // Fake leaf
        let fake = BigUint::from(100u32);
        assert!(!verify_merkle(&fake, 2, &proof, &root));

        // Wrong index
        assert!(!verify_merkle(&v2, 0,&proof, &root));
    }
}