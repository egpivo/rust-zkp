use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSummary {
    pub id: u32,
    pub balance: u64,
    pub nonce: u64,
}
