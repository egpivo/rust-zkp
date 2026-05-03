use crate::transcript::fs_piece;
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
    #[serde(with = "crate::serde_helpers::biguint_string")]
    pub r: BigUint, // randomly commit g^k mod p
    #[serde(with = "crate::serde_helpers::biguint_string")]
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

pub fn challenge_sigma_dl(g: &BigUint, c: &BigUint, r: &BigUint, p: &BigUint) -> BigUint {
    let mut hasher = Sha256::new();
    // Add domain tag
    hasher.update(b"sigma-dl-v1\0");

    // Add pieces
    fs_piece(&mut hasher, 1, &g.to_bytes_be());
    fs_piece(&mut hasher, 2, &c.to_bytes_be());
    fs_piece(&mut hasher, 3, &r.to_bytes_be());
    fs_piece(&mut hasher, 4, &p.to_bytes_be());
    BigUint::from_bytes_be(&hasher.finalize()) % p
}

pub fn challenge_tx(
    g: &BigUint,
    pubkey: &BigUint,
    r: &BigUint,
    p: &BigUint,
    message: &[u8],
) -> BigUint {
    let mut hasher = Sha256::new();
    // Add domain tag
    hasher.update(b"tx-challenge-v1\0");
    // Add pieces
    fs_piece(&mut hasher, 1, &g.to_bytes_be());
    fs_piece(&mut hasher, 2, &pubkey.to_bytes_be());
    fs_piece(&mut hasher, 3, &r.to_bytes_be());
    fs_piece(&mut hasher, 4, &p.to_bytes_be());
    fs_piece(&mut hasher, 5, message);
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

    #[test]
    fn test_challenge_sigma_dl_is_deterministic() {
        let p = BigUint::from(223u32);
        let g = BigUint::from(4u32);
        let c = BigUint::from(3u32);
        let r = BigUint::from(11u32);
        let challenge1 = challenge_sigma_dl(&g, &c, &r, &p);
        let challenge2 = challenge_sigma_dl(&g, &c, &r, &p);
        assert_eq!(challenge1, challenge2);
    }

    #[test]
    fn test_challenge_sigma_dl_changes_when_transcript_changes() {
        let p = BigUint::from(223u32);
        let g = BigUint::from(4u32);
        let c = BigUint::from(3u32);
        let r = BigUint::from(11u32);

        let baseline = challenge_sigma_dl(&g, &c, &r, &p);
        assert_ne!(baseline, challenge_sigma_dl(&g, &(&c + 1u32), &r, &p));
        assert_ne!(baseline, challenge_sigma_dl(&g, &c, &(&r + 1u32), &p));
        assert_ne!(baseline, challenge_sigma_dl(&g, &c, &r, &(&p + 1u32)));
    }

    #[test]
    fn test_challenge_tx_is_deterministic() {
        let p = BigUint::from(223u32);
        let g = BigUint::from(4u32);
        let pubkey = BigUint::from(3u32);
        let r = BigUint::from(11u32);
        let message = b"Hello, world!";

        let e0 = challenge_tx(&g, &pubkey, &r, &p, message);
        let e1 = challenge_tx(&g, &pubkey, &r, &p, message);
        assert_eq!(e0, e1);
    }

    #[test]
    fn test_challenge_tx_domain_separated_from_sigma_dl() {
        let p = BigUint::from(223u32);
        let g = BigUint::from(4u32);
        let c = BigUint::from(3u32);
        let r = BigUint::from(11u32);

        let e_sigma_dl = challenge_sigma_dl(&g, &c, &r, &p);
        let e_tx = challenge_tx(&g, &c, &r, &p, &[]);
        assert_ne!(e_sigma_dl, e_tx);
    }
}
