use num_bigint::BigUint;
use crate::sigma::Proof;

pub struct Transaction {
    pub from: u32,
    pub to: u32,
    pub amount: u64,
    pub proof: Proof,
    pub challenge_e: BigUint,  
}
