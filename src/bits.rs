use crate::transcript::fs_piece;
use num_bigint::{BigUint, RandBigInt};
use rand::thread_rng;
use sha2::{Digest, Sha256};

pub fn to_bits(n: &BigUint, num_bits: usize) -> Vec<BigUint> {
    (0..num_bits)
        .map(|i| {
            if n.bit(i as u64) {
                BigUint::from(1u32)
            } else {
                BigUint::from(0u32)
            }
        })
        .collect()
}

fn mod_inv(a: &BigUint, p: &BigUint) -> BigUint {
    a.modpow(&(p - BigUint::from(2u32)), p)
}

fn sub_mod(total: &BigUint, sub: &BigUint, order: &BigUint) -> BigUint {
    let sub = sub % order;
    let total = total % order;

    if total >= sub {
        &total - &sub
    } else {
        order - (&sub - &total)
    }
}

/// Transcript for Fiat–Shamir in the Pedersen bit-OR proof (keeps clippy happy vs 8 loose args).
struct BitOrFsCtx<'a> {
    g: &'a BigUint,
    h: &'a BigUint,
    p: &'a BigUint,
    order: &'a BigUint,
    y0: &'a BigUint,
    y1: &'a BigUint,
    a0: &'a BigUint,
    a1: &'a BigUint,
}

fn hash_challenge_bit_or(ctx: &BitOrFsCtx<'_>) -> BigUint {
    let mut hasher = Sha256::new();
    // Add domain tag
    hasher.update(b"pederson-bit-or-v1\0");
    // Add pieces
    fs_piece(&mut hasher, 1, &ctx.g.to_bytes_be());
    fs_piece(&mut hasher, 2, &ctx.h.to_bytes_be());
    fs_piece(&mut hasher, 3, &ctx.p.to_bytes_be());
    fs_piece(&mut hasher, 4, &ctx.y0.to_bytes_be());
    fs_piece(&mut hasher, 5, &ctx.y1.to_bytes_be());
    fs_piece(&mut hasher, 6, &ctx.a0.to_bytes_be());
    fs_piece(&mut hasher, 7, &ctx.a1.to_bytes_be());
    BigUint::from_bytes_be(&hasher.finalize()) % ctx.order
}

#[derive(Debug, Clone)]
pub struct BitOrProof {
    pub a0: BigUint,
    pub a1: BigUint,
    pub s0: BigUint,
    pub s1: BigUint,

    pub e_fake: BigUint,
    pub fake_is_branch1: bool,
}

pub fn prove_bit_or(
    b: &BigUint,
    r: &BigUint,
    c_bit: &BigUint,
    g: &BigUint,
    h: &BigUint,
    p: &BigUint,
) -> BitOrProof {
    assert!(*b == BigUint::from(0u32) || *b == BigUint::from(1u32));
    let order = p - BigUint::from(1u32);
    let g_inv = mod_inv(g, p);
    let y0 = c_bit % p;
    let y1 = (&y0 * &g_inv) % p;
    let mut rng = thread_rng();
    if *b == BigUint::from(0u32) {
        // real: Y0 = h^r；fake: branch1
        let t0 = rng.gen_biguint_below(&order);
        let a0 = h.modpow(&t0, p);
        let e1_fake = rng.gen_biguint_below(&order);
        let s1_fake = rng.gen_biguint_below(&order);
        let a1 = (h.modpow(&s1_fake, p) * mod_inv(&y1.modpow(&e1_fake, p), p)) % p;
        let e_total = hash_challenge_bit_or(&BitOrFsCtx {
            g,
            h,
            p,
            order: &order,
            y0: &y0,
            y1: &y1,
            a0: &a0,
            a1: &a1,
        });
        let e0 = sub_mod(&e_total, &e1_fake, &order);
        let s0 = &t0 + &(&e0 * r);
        BitOrProof {
            a0,
            a1,
            s0,
            s1: s1_fake,
            e_fake: e1_fake,
            fake_is_branch1: true,
        }
    } else {
        // real：Y1 = h^r；fake：branch0
        let t1 = rng.gen_biguint_below(&order);
        let a1 = h.modpow(&t1, p);
        let e0_fake = rng.gen_biguint_below(&order);
        let s0_fake = rng.gen_biguint_below(&order);
        let a0 = (h.modpow(&s0_fake, p) * mod_inv(&y0.modpow(&e0_fake, p), p)) % p;
        let e_total = hash_challenge_bit_or(&BitOrFsCtx {
            g,
            h,
            p,
            order: &order,
            y0: &y0,
            y1: &y1,
            a0: &a0,
            a1: &a1,
        });
        let e1 = sub_mod(&e_total, &e0_fake, &order);
        let s1 = &t1 + &(&e1 * r);
        BitOrProof {
            a0,
            a1,
            s0: s0_fake,
            s1,
            e_fake: e0_fake,
            fake_is_branch1: false,
        }
    }
}

pub fn verify_bit_or(
    proof: &BitOrProof,
    c_bit: &BigUint,
    g: &BigUint,
    h: &BigUint,
    p: &BigUint,
) -> bool {
    let order = p - BigUint::from(1u32);
    let g_inv = mod_inv(g, p);
    let y0 = c_bit % p;
    let y1 = (&y0 * &g_inv) % p;
    let e_total = hash_challenge_bit_or(&BitOrFsCtx {
        g,
        h,
        p,
        order: &order,
        y0: &y0,
        y1: &y1,
        a0: &proof.a0,
        a1: &proof.a1,
    });
    let (e0, e1) = if proof.fake_is_branch1 {
        let e1 = &proof.e_fake % &order;
        let e0 = sub_mod(&e_total, &e1, &order);
        (e0, e1)
    } else {
        let e0 = &proof.e_fake % &order;
        let e1 = sub_mod(&e_total, &e0, &order);
        (e0, e1)
    };
    let lhs0 = h.modpow(&proof.s0, p);
    let rhs0 = (&proof.a0 * y0.modpow(&e0, p)) % p;
    let lhs1 = h.modpow(&proof.s1, p);
    let rhs1 = (&proof.a1 * y1.modpow(&e1, p)) % p;
    lhs0 == rhs0 && lhs1 == rhs1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commitment::commit;
    use num_bigint::RandBigInt;

    #[test]
    fn test_to_bits() {
        let expected: Vec<BigUint> = vec![1u32, 0, 1, 1].into_iter().map(BigUint::from).collect();
        assert_eq!(to_bits(&BigUint::from(13u32), 4), expected);
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
        let c_bits: Vec<BigUint> = bits
            .iter()
            .zip(&r_bits)
            .map(|(b, r)| commit(b, r, &g, &h, &p))
            .collect();

        // r_delta == sum of r_i * 2^i
        let r_delta: BigUint = (0..8).map(|i| &r_bits[i] * BigUint::from(1u32 << i)).sum();

        // Verifier: compute product of C_i^(2^i)
        let mut lhs = BigUint::from(1u32);
        for (i, c_i) in c_bits.iter().enumerate() {
            lhs = (lhs * c_i.modpow(&BigUint::from(1u32 << i), &p)) % &p;
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
        for (i, b) in bits.iter().enumerate() {
            reconstructed += b * BigUint::from(1u32 << i);
        }
        assert_eq!(delta, reconstructed);
    }

    /// Pedersen bit commitment C = g^b h^r with b ∈ {0,1}; OR proof should verify.
    #[test]
    fn test_bit_or_valid_for_bit_zero() {
        let p = BigUint::from(223u32);
        let g = BigUint::from(4u32);
        let h = BigUint::from(3u32);
        let mut rng = rand::thread_rng();
        let r = rng.gen_biguint_below(&p);
        let b = BigUint::from(0u32);
        let c = commit(&b, &r, &g, &h, &p);
        let proof = prove_bit_or(&b, &r, &c, &g, &h, &p);
        assert!(verify_bit_or(&proof, &c, &g, &h, &p));
    }

    #[test]
    fn test_bit_or_valid_for_bit_one() {
        let p = BigUint::from(223u32);
        let g = BigUint::from(4u32);
        let h = BigUint::from(3u32);
        let mut rng = rand::thread_rng();
        let r = rng.gen_biguint_below(&p);
        let b = BigUint::from(1u32);
        let c = commit(&b, &r, &g, &h, &p);
        let proof = prove_bit_or(&b, &r, &c, &g, &h, &p);
        assert!(verify_bit_or(&proof, &c, &g, &h, &p));
    }

    #[test]
    fn test_bit_or_rejects_tampered_response() {
        let p = BigUint::from(223u32);
        let g = BigUint::from(4u32);
        let h = BigUint::from(3u32);
        let mut rng = rand::thread_rng();
        let r = rng.gen_biguint_below(&p);
        let b = BigUint::from(1u32);
        let c = commit(&b, &r, &g, &h, &p);
        let mut proof = prove_bit_or(&b, &r, &c, &g, &h, &p);
        proof.s0 += BigUint::from(1u32);
        assert!(!verify_bit_or(&proof, &c, &g, &h, &p));
    }

    #[test]
    fn test_bit_or_rejects_wrong_commitment_for_proof() {
        let p = BigUint::from(223u32);
        let g = BigUint::from(4u32);
        let h = BigUint::from(3u32);
        let mut rng = rand::thread_rng();
        let r = rng.gen_biguint_below(&p);
        let b = BigUint::from(0u32);
        let c = commit(&b, &r, &g, &h, &p);
        let proof = prove_bit_or(&b, &r, &c, &g, &h, &p);
        let other_c = commit(&BigUint::from(1u32), &r, &g, &h, &p);
        assert!(!verify_bit_or(&proof, &other_c, &g, &h, &p));
    }

    /// Ex3 path: homomorphic product + each bit commitment has a valid bit-OR proof.
    #[test]
    fn test_threshold_with_bit_or_per_committed_bit() {
        let p = BigUint::from(223u32);
        let g = BigUint::from(4u32);
        let h = BigUint::from(3u32);
        let mut rng = rand::thread_rng();

        let v = BigUint::from(10u32);
        let t = BigUint::from(7u32);
        let delta = &v - &t;
        let bits = to_bits(&delta, 8);
        let r_bits: Vec<BigUint> = (0..8).map(|_| rng.gen_biguint_below(&p)).collect();
        let c_bits: Vec<BigUint> = bits
            .iter()
            .zip(&r_bits)
            .map(|(b, r)| commit(b, r, &g, &h, &p))
            .collect();

        for (b, (c_i, r_i)) in bits.iter().zip(c_bits.iter().zip(&r_bits)) {
            let proof = prove_bit_or(b, r_i, c_i, &g, &h, &p);
            assert!(
                verify_bit_or(&proof, c_i, &g, &h, &p),
                "bit-OR must hold for each committed bit"
            );
        }

        let r_delta: BigUint = (0..8).map(|i| &r_bits[i] * BigUint::from(1u32 << i)).sum();
        let mut lhs = BigUint::from(1u32);
        for (i, c_i) in c_bits.iter().enumerate() {
            lhs = (lhs * c_i.modpow(&BigUint::from(1u32 << i), &p)) % &p;
        }
        let rhs = commit(&delta, &r_delta, &g, &h, &p);
        assert_eq!(lhs, rhs);
    }
}
