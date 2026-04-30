use clap::{Parser, Subcommand};
use num_bigint::BigUint;
use serde_json::json;
use zkp::sigma::{prove_commit, prove_response, challenge_for_tx, Proof};
use zkp::transaction::Transaction;


#[derive(Parser)]
#[command(name = "zkp-cli")]
#[command(about = "ZKP rollup client")]
struct Cli {
    #[arg(long, default_value = "http://localhost:3000")]
    server: String,

    #[command(subcommand)]
    command: Command
}

#[derive(Subcommand)]
enum Command {
    StateRoot,

    // Get a balance
    Balance {
        id: u32,
    },

    // Register a new account
    Register {
        #[arg(long)]
        id: u32,
        #[arg(long)]
        balance: u64,
        #[arg(long)]
        secret: u64,
    },

    // Send a transaction
    Send {
        #[arg(long)]
        from: u32,
        #[arg(long)]
        to: u32,
        #[arg(long)]
        amount: u64,
        #[arg(long)]
        nonce: u64,
        #[arg(long)]
        secret: u64,
    },
}


#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::StateRoot => {
            let url = format!("{}/state-root", cli.server);
            let resp = reqwest::get(&url).await.unwrap().text().await.unwrap();
            println!("{}", resp);
        }
        Command::Balance { id } => {
            let url = format!("{}/balance/{}", cli.server, id);
            let resp = reqwest::get(&url).await.unwrap().text().await.unwrap();
            println!("{}", resp);
        }

        Command::Register { id, balance, secret } => {
            // System parameters
            let p = BigUint::from(223u32);
            let g = BigUint::from(4u32);

            // Compute pubkey = g^secret mod p
            let secret_big = BigUint::from(secret);
            let pubkey = g.modpow(&secret_big, &p);

            // POST to /accounts
            let url = format!("{}/accounts", cli.server);
            let body = json!({
                "id": id,
                "balance": balance,
                "pubkey": pubkey,
            });

            let client = reqwest::Client::new();
            let resp = client.post(&url)
                .json(&body)
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();

            println!("{}", resp);
        }
        Command::Send { from, to, amount, nonce, secret } => {
            // System params
            let p = BigUint::from(223u32);
            let g = BigUint::from(4u32);

            // Compute pubkey
            let secret_big = BigUint::from(secret);
            let pubkey = g.modpow(&secret_big, &p);

            // Build the unsigned message
            let mut msg = vec![];
            msg.extend(from.to_be_bytes());
            msg.extend(to.to_be_bytes());
            msg.extend(amount.to_be_bytes());
            msg.extend(nonce.to_be_bytes());    
            
            // Sign (sigma protocal)
            let (k, r) = prove_commit(&g, &g);
            let e = challenge_for_tx(&g, &pubkey, &r, &p, &msg);
            let z = prove_response(&k, &e, &secret_big);
            let proof = Proof { r, z };

            // Build full tx
            let tx = Transaction {
                from,
                to,
                amount,
                nonce,
                proof,
                challenge_e: e,
            };

            // POST /tx
            let url = format!("{}/tx", cli.server);
            let client = reqwest::Client::new();
            let resp = client.post(&url)
                .json(&tx)
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();


            println!("{}", resp );
        }
    }
}