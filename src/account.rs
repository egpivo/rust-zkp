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
}

