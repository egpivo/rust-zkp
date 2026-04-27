use num_bigint::BigUint;
use sha2::{Sha256, Digest};


#[derive(Debug, Clone)]
pub struct Account {
    pub id: u32,
    pub balance: u64,
    pub nonce: u64,
}

impl Account {
    pub fn new(id: u32, balance: u64) -> Self {
        Self { id, balance, nonce: 0 }
    }

    pub fn hash(&self) -> BigUint {
        let mut hasher = Sha256::new();
        hasher.update(self.id.to_be_bytes());
        hasher.update(self.balance.to_be_bytes());     
        hasher.update(self.nonce.to_be_bytes());  
        BigUint::from_bytes_be(&hasher.finalize())
    }
}

