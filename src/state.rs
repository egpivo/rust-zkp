use std::collections::HashMap;
use num_bigint::BigUint;
use crate::account::Account;
use crate::merkle::build_tree;
use crate::sigma::Proof;


#[derive(Debug)]
pub struct State {
    pub accounts: HashMap<u32, Account>,
}

impl State {
    pub fn new() -> Self {
        Self { accounts: HashMap::new() }
    }

    pub fn add_account(&mut self, account: Account) {
        self.accounts.insert(account.id, account);
    }

    pub fn transfer(
        &mut self,
        from: u32,
        to: u32,
        amount: u64,
        proof: &Proof,
        challenge_e: &BigUint,
        g: &BigUint,
        p: &BigUint,
    ) -> Result<(), String> {
        let from_balance = self.accounts.get(&from)
            .ok_or("from account not found")?
            .balance;

        if from_balance < amount {
            return Err("insufficient balance".to_string());
        }
        if !self.accounts.contains_key(&to) {
            return Err("to account not found".to_string());
        }

        let from_pubkey = &self.accounts[&from].pubkey;
        if !Proof::verify(proof, from_pubkey, challenge_e, g, p) {
            return Err("invalid signature".to_string());
        }
        
        let from_account = self.accounts.get_mut(&from).unwrap();
        from_account.balance -= amount;
        from_account.nonce += 1;
        
        let to_account = self.accounts.get_mut(&to).unwrap();     
        to_account.balance += amount;

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
    use crate::sigma::{prove_commit, prove_response, challenge, Proof};

    #[test]
    fn test_transfer_success() {
        let p = BigUint::from(123u32);
        let g = BigUint::from(4u32);
        let secret = BigUint::from(1232u32);
        let pubkey = g.modpow(&secret, &p);
        let (k, r) = prove_commit(&g, &p);
        let e = challenge(&g, &pubkey, &r, &p);
        let z = prove_response(&k, &e, &secret);
        let proof = Proof { r, z };

        let mut state = State::new();
        state.add_account(Account::new(1, 100, pubkey.clone()));
        state.add_account(Account::new(2, 50, pubkey.clone()));

        state.transfer(1, 2, 30, &proof, &e, &g, &p).unwrap();

        assert_eq!(state.accounts[&1].balance, 70);
        assert_eq!(state.accounts[&2].balance, 80);  
        assert_eq!(state.accounts[&1].nonce, 1);    
    }

    #[test]
    fn test_insufficient_balance() {
        let p = BigUint::from(123u32);
        let g = BigUint::from(4u32);
        let secret = BigUint::from(1232u32);
        let pubkey = g.modpow(&secret, &p);
        let (k, r) = prove_commit(&g, &p);
        let e = challenge(&g, &pubkey, &r, &p);
        let z = prove_response(&k, &e, &secret);
        let proof = Proof { r, z };

        let mut state = State::new();
        state.add_account(Account::new(1, 10, pubkey.clone()));
        state.add_account(Account::new(2, 0, pubkey.clone()));
        
        let result = state.transfer(1, 2, 100, &proof, &e, &g, &p);
        assert!(result.is_err());
        assert_eq!(state.accounts[&1].balance, 10);
    }

    #[test]
    fn test_to_account_missing() {
        let p = BigUint::from(123u32);
        let g = BigUint::from(4u32);
        let secret = BigUint::from(1232u32);
        let pubkey = g.modpow(&secret, &p);
        let (k, r) = prove_commit(&g, &p);
        let e = challenge(&g, &pubkey, &r, &p);
        let z = prove_response(&k, &e, &secret);
        let proof = Proof { r, z };

        let mut state = State::new();
        state.add_account(Account::new(1, 100, pubkey.clone()));

        let result = state.transfer(1, 999, 10, &proof, &e, &g, &p);
        assert!(result.is_err());

        assert_eq!(state.accounts[&1].balance, 100);
        assert_eq!(state.accounts[&1].nonce, 0);
    }

    #[test]
    fn test_state_root_deterministic() {
        let p = BigUint::from(123u32);
        let g = BigUint::from(4u32);
        let secret = BigUint::from(1232u32);
        let pubkey = g.modpow(&secret, &p);
        
        let mut state1 = State::new();
        state1.add_account(Account::new(1, 100, pubkey.clone()));
        state1.add_account(Account::new(2, 10, pubkey.clone()));
        state1.add_account(Account::new(3, 200, pubkey.clone()));
        state1.add_account(Account::new(4, 20, pubkey.clone()));
        
        let mut state2 = State::new();
        state2.add_account(Account::new(3, 200, pubkey.clone()));
        state2.add_account(Account::new(4, 20, pubkey.clone()));        
        state2.add_account(Account::new(1, 100, pubkey.clone()));
        state2.add_account(Account::new(2, 10, pubkey.clone()));

        assert_eq!(state1.state_root(), state2.state_root());
    }

    #[test]
    fn test_state_root_changes_after_transfer() {
        let p = BigUint::from(123u32);
        let g = BigUint::from(4u32);
        let secret = BigUint::from(1232u32);
        let pubkey = g.modpow(&secret, &p);
        let (k, r) = prove_commit(&g, &p);
        let e = challenge(&g, &pubkey, &r, &p);
        let z = prove_response(&k, &e, &secret);
        let proof = Proof { r, z };
      
        let mut state = State::new();
        state.add_account(Account::new(3, 200, pubkey.clone()));
        state.add_account(Account::new(4, 20, pubkey.clone()));        
        state.add_account(Account::new(1, 100, pubkey.clone()));
        state.add_account(Account::new(2, 10, pubkey.clone()));  
        
        let root_before = state.state_root();
        state.transfer(1, 2, 30, &proof, &e, &g, &p).unwrap();
        let root_after = state.state_root();

        assert_ne!(root_before, root_after);
    }
}