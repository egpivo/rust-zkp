use sled::Db;
use crate::account::Account;

pub struct Storage {
    db: Db,
}

impl Storage {
    pub fn open(path: &str) -> sled::Result<Self> {
        let db = sled::open(path)?;
        Ok(Self { db })
    }

    pub fn save_account(&self, account: &Account) -> sled::Result<()> {
        let key = format!("account:{}", account.id);
        let value = bincode::serialize(account).unwrap();
        self.db.insert(key, value)?;
        Ok(())
    }

    pub fn save_accounts(&self, accounts: &[&Account]) -> sled::Result<()> {
        let mut batch = sled::Batch::default();
        for account in accounts {
            let key = format!("account:{}", account.id);
            let value = bincode::serialize(account).unwrap();
            batch.insert(key.as_bytes(), value);
        }
        self.db.apply_batch(batch)?;
        Ok(())
    }

    pub fn load_all_accounts(&self) -> sled::Result<Vec<Account>> {
        let prefix = "account:";
        let mut accounts = vec![];
        for item in self.db.scan_prefix(prefix) {
            let (_, value) = item?;
            accounts.push(bincode::deserialize(&value).unwrap());
        }
        Ok(accounts)
    }
}
