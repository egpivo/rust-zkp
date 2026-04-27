use std::collections::HashMap;
use num_bigint::BigUint;
use crate::account::Account;
use crate::merkle::build_tree;


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

    pub fn transfer(&mut self, from: u32, to: u32, amount: u64) -> Result<(), String> {
        let from_balance = self.accounts.get(&from)
            .ok_or("from account not found")?
            .balance;

        if from_balance < amount {
            return Err("insufficient balance".to_string());
        }
        if !self.accounts.contains_key(&to) {
            return Err("to account not found".to_string());
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

    #[test]
    fn test_transfer_success() {
        let mut state = State::new();
        state.add_account(Account::new(1, 100));
        state.add_account(Account::new(2, 50));

        state.transfer(1, 2, 30).unwrap();

        assert_eq!(state.accounts[&1].balance, 70);
        assert_eq!(state.accounts[&2].balance, 80);  
        assert_eq!(state.accounts[&1].nonce, 1);    
    }

    #[test]
    fn test_insufficient_balance() {
        let mut state = State::new();
        state.add_account(Account::new(1, 10));
        state.add_account(Account::new(2, 0));
        
        let result = state.transfer(1, 2, 100);
        assert!(result.is_err());
        assert_eq!(state.accounts[&1].balance, 10);
    }

    #[test]
    fn test_to_account_missing() {
        let mut state = State::new();
        state.add_account(Account::new(1, 100));

        let result = state.transfer(1, 999, 10);
        assert!(result.is_err());

        assert_eq!(state.accounts[&1].balance, 100);
        assert_eq!(state.accounts[&1].nonce, 0);
    }

    #[test]
    fn test_state_root_deterministic() {
        let mut state1 = State::new();
        state1.add_account(Account::new(1, 100));
        state1.add_account(Account::new(2, 10));
        state1.add_account(Account::new(3, 200));
        state1.add_account(Account::new(4, 20));
        
        let mut state2 = State::new();
        state2.add_account(Account::new(3, 200));
        state2.add_account(Account::new(4, 20));        
        state2.add_account(Account::new(1, 100));
        state2.add_account(Account::new(2, 10));

        assert_eq!(state1.state_root(), state2.state_root());
    }

    #[test]
    fn test_state_root_changes_after_transfer() {
        let mut state = State::new();
        state.add_account(Account::new(3, 200));
        state.add_account(Account::new(4, 20));        
        state.add_account(Account::new(1, 100));
        state.add_account(Account::new(2, 10));  
        
        let root_before = state.state_root();
        state.transfer(1, 2, 30).unwrap();
        let root_after = state.state_root();

        assert_ne!(root_before, root_after);
    }
}