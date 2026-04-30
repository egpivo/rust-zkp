use num_bigint::BigUint;
use sha2::{Sha256, Digest};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: u32,
    pub balance: u64,
    pub nonce: u64,
    pub pubkey: BigUint,
}

impl Account {
    pub fn new(id: u32, balance: u64, pubkey: BigUint) -> Self {
        Self { id, balance, nonce: 0, pubkey }
    }

    pub fn hash(&self) -> BigUint {
        let mut hasher = Sha256::new();
        hasher.update(self.id.to_be_bytes());
        hasher.update(self.balance.to_be_bytes());     
        hasher.update(self.nonce.to_be_bytes());
        hasher.update(self.pubkey.to_bytes_be());        
        BigUint::from_bytes_be(&hasher.finalize())
    }
}

