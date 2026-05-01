use crate::sigma::Proof;
use num_bigint::BigUint;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub from: u32,
    pub to: u32,
    pub amount: u64,
    pub nonce: u64,
    pub proof: Proof,
    #[serde(with = "crate::serde_helpers::biguint_string")]
    pub challenge_e: BigUint,
}

impl Transaction {
    pub fn message_to_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![];
        bytes.extend(self.from.to_be_bytes());
        bytes.extend(self.to.to_be_bytes());
        bytes.extend(self.amount.to_be_bytes());
        bytes.extend(self.nonce.to_be_bytes());
        bytes
    }
}
