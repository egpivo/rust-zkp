use crate::transaction::Transaction;
use num_bigint::BigUint;

pub struct Batch {
    pub txs: Vec<Transaction>,
    pub state_root_before: BigUint,
    pub state_root_after: BigUint,
}
