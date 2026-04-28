use std::collections::HashMap;
use num_bigint::BigUint;
use crate::account::Account;
use crate::merkle::build_tree;
use crate::sigma::{Proof, challenge_for_tx};
use crate::transaction::Transaction;

#[derive(Debug)]
pub struct State {
    pub accounts: HashMap<u32, Account>,
    pub g: BigUint,
    pub p: BigUint,
}

impl State {
    pub fn new(p: BigUint, g: BigUint) -> Self {
        Self { accounts: HashMap::new(), p, g }
    }

    pub fn add_account(&mut self, account: Account) {
        self.accounts.insert(account.id, account);
    }

    pub fn apply_tx(&mut self, tx: &Transaction) -> Result<(), String> {
        let from_balance = self.accounts.get(&tx.from)
            .ok_or("from account not found")?
            .balance;

        if from_balance < tx.amount {
            return Err("insufficient balance".to_string());
        }
        if !self.accounts.contains_key(&tx.to) {
            return Err("to account not found".to_string());
        }

        let from_pubkey = &self.accounts[&tx.from].pubkey;
        let e = challenge_for_tx(&self.g, &from_pubkey, &tx.proof.r, &self.p, &tx.message_to_bytes());
        if !Proof::verify(&tx.proof, from_pubkey, &e, &self.g, &self.p) {
            return Err("invalid signature".to_string());
        }
        
        let from_account = self.accounts.get_mut(&tx.from).unwrap();
        from_account.balance -= tx.amount;
        from_account.nonce += 1;
        
        let to_account = self.accounts.get_mut(&tx.to).unwrap();     
        to_account.balance += tx.amount;

        Ok(())
    }

    pub fn state_root(&self) -> BigUint {
        let mut ids: Vec<&u32> = self.accounts.keys().collect();
        ids.sort();

        let leaves: Vec<BigUint> = ids.iter()
            .map(|id| self.accounts[id].hash())
            .collect();

        build_tree(leaves)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sigma::{prove_commit, prove_response};
    use crate::transaction::Transaction;

    struct TestCtx {
        p: BigUint,
        g: BigUint,
        secret: BigUint,
        pubkey: BigUint,
    }

    fn sign_tx(
        p: &BigUint,    
        g: &BigUint,   
        secret: &BigUint,
        pubkey: &BigUint,
        tx_msg: &[u8],
    ) -> (Proof, BigUint) {
        let (k, r) = prove_commit(g, p);
        let e = challenge_for_tx(&g, &pubkey, &r, &p, &tx_msg);
        let z = prove_response(&k, &e, secret);
        (Proof { r, z }, e)
    }

    fn test_setup()-> TestCtx {
        let p = BigUint::from(223u32);
        let g = BigUint::from(4u32);
        let secret = BigUint::from(2232u32);
        let pubkey = g.modpow(&secret, &p);
        TestCtx { p, g, secret, pubkey}
    }

    #[test]
    fn test_apply_tx_success() {
        let TestCtx { p, g, secret, pubkey} = test_setup();
        let mut state = State::new(p.clone(), g.clone());
        state.add_account(Account::new(1, 100, pubkey.clone()));
        state.add_account(Account::new(2, 50, pubkey.clone()));

        let from = 1u32;
        let to = 2u32;
        let amount = 30u64;
        let nonce = 1u64;

        let mut msg = vec![];
        msg.extend(from.to_be_bytes());
        msg.extend(to.to_be_bytes());
        msg.extend(amount.to_be_bytes());
        msg.extend(nonce.to_be_bytes());        

        let (proof, e)= sign_tx(&p, &g, &secret, &pubkey, &msg);
        let tx = Transaction {
            from,
            to,
            amount,
            nonce,            
            proof,
            challenge_e: e,
        };  
        state.apply_tx(&tx).unwrap();

        assert_eq!(state.accounts[&1].balance, 70);
        assert_eq!(state.accounts[&2].balance, 80);  
        assert_eq!(state.accounts[&1].nonce, 1);    
    }

    #[test]
    fn test_insufficient_balance() {
 
        let TestCtx { p, g, secret, pubkey} = test_setup();
        let mut state = State::new(p.clone(), g.clone());
        state.add_account(Account::new(1, 10, pubkey.clone()));
        state.add_account(Account::new(2, 0, pubkey.clone()));

        let from = 1u32;
        let to = 2u32;
        let amount = 100u64;
        let nonce = 1u64;

        let mut msg = vec![];
        msg.extend(from.to_be_bytes());
        msg.extend(to.to_be_bytes());
        msg.extend(amount.to_be_bytes());
        msg.extend(nonce.to_be_bytes());        

        let (proof, e)= sign_tx(&p, &g, &secret, &pubkey, &msg);
        let tx = Transaction {
            from: 1,
            to: 2,
            amount: 100,
            nonce: 1, 
            proof,
            challenge_e: e,
        };       

        let result = state.apply_tx(&tx);
        assert!(result.is_err());
        assert_eq!(state.accounts[&1].balance, 10);
    }

    #[test]
    fn test_to_account_missing() {
      
        let TestCtx { p, g, secret, pubkey} = test_setup();
        let mut state = State::new(p.clone(), g.clone());
        state.add_account(Account::new(1, 100, pubkey.clone()));
        let from = 1u32;
        let to = 2u32;
        let amount = 100u64;
        let nonce = 1u64;

        let mut msg = vec![];
        msg.extend(from.to_be_bytes());
        msg.extend(to.to_be_bytes());
        msg.extend(amount.to_be_bytes());
        msg.extend(nonce.to_be_bytes());        

        let (proof, e)= sign_tx(&p, &g, &secret, &pubkey, &msg);      
        let tx = Transaction {
            from: 1,
            to: 2,
            amount: 100,
            nonce: 1,
            proof,
            challenge_e: e,
        };  

        let result = state.apply_tx(&tx);
        assert!(result.is_err());

        assert_eq!(state.accounts[&1].balance, 100);
        assert_eq!(state.accounts[&1].nonce, 0);
    }

    #[test]
    fn test_state_root_deterministic() {       
        let TestCtx { p, g, secret, pubkey} = test_setup();
        
        let mut state1 = State::new(p.clone(), g.clone());
        state1.add_account(Account::new(1, 100, pubkey.clone()));
        state1.add_account(Account::new(2, 10, pubkey.clone()));
        state1.add_account(Account::new(3, 200, pubkey.clone()));
        state1.add_account(Account::new(4, 20, pubkey.clone()));
        
        let mut state2 = State::new(p, g);
        state2.add_account(Account::new(3, 200, pubkey.clone()));
        state2.add_account(Account::new(4, 20, pubkey.clone()));        
        state2.add_account(Account::new(1, 100, pubkey.clone()));
        state2.add_account(Account::new(2, 10, pubkey.clone()));

        assert_eq!(state1.state_root(), state2.state_root());
    }

    #[test]
    fn test_state_root_changes_after_apply_tx() {
        
        let TestCtx { p, g, secret, pubkey} = test_setup();
        let mut state = State::new(p.clone(), g.clone());
        state.add_account(Account::new(3, 200, pubkey.clone()));
        state.add_account(Account::new(4, 20, pubkey.clone()));        
        state.add_account(Account::new(1, 100, pubkey.clone()));
        state.add_account(Account::new(2, 10, pubkey.clone()));  
        
        let root_before = state.state_root();
        let from = 1u32;
        let to = 2u32;
        let amount = 100u64;
        let nonce = 1u64;

        let mut msg = vec![];
        msg.extend(from.to_be_bytes());
        msg.extend(to.to_be_bytes());
        msg.extend(amount.to_be_bytes());
        msg.extend(nonce.to_be_bytes());        

        let (proof, e)= sign_tx(&p, &g, &secret, &pubkey, &msg);   
        let tx = Transaction {
            from: 1,
            to: 2,
            amount: 100,
            nonce: 1,
            proof,
            challenge_e: e,
        };    
        state.apply_tx(&tx).unwrap();
        let root_after = state.state_root();

        assert_ne!(root_before, root_after);
    }
}