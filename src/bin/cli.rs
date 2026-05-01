use clap::{Parser, Subcommand};
use num_bigint::BigUint;
use serde_json::json;
use zkp::dto::AccountSummary;
use zkp::sigma::{Proof, challenge_for_tx, prove_commit, prove_response};
use zkp::transaction::Transaction;

#[derive(Parser)]
#[command(name = "zkp-cli")]
#[command(about = "ZKP rollup client")]
struct Cli {
    #[arg(long, default_value = "http://localhost:3000")]
    server: String,

    #[command(subcommand)]
    command: Command,
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

#[derive(serde::Deserialize)]
struct ParamsResponse {
    #[serde(with = "zkp::serde_helpers::biguint_string")]
    p: BigUint,
    #[serde(with = "zkp::serde_helpers::biguint_string")]
    g: BigUint,
}

async fn fetch_params(server: &str) -> ParamsResponse {
    let url = format!("{}/params", server);
    reqwest::get(&url)
        .await
        .unwrap()
        .json::<ParamsResponse>()
        .await
        .unwrap()
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
            let url = format!("{}/accounts/{}", cli.server, id);
            let resp: AccountSummary = reqwest::get(&url).await.unwrap().json().await.unwrap();
            println!("{}", resp.balance);
        }

        Command::Register {
            id,
            balance,
            secret,
        } => {
            let params = fetch_params(&cli.server).await;
            // Compute pubkey = g^secret mod p
            let secret_big = BigUint::from(secret);
            let pubkey = params.g.modpow(&secret_big, &params.p);

            // POST to /accounts
            let url = format!("{}/accounts", cli.server);
            let body = json!({
                "id": id,
                "balance": balance,
                "pubkey": pubkey.to_string(),
            });

            let client = reqwest::Client::new();
            let resp = client
                .post(&url)
                .json(&body)
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();

            println!("{}", resp);
        }
        Command::Send {
            from,
            to,
            amount,
            nonce,
            secret,
        } => {
            let params = fetch_params(&cli.server).await;
            let secret_big = BigUint::from(secret);
            let pubkey = params.g.modpow(&secret_big, &params.p);

            // Build the unsigned message
            let mut msg = vec![];
            msg.extend(from.to_be_bytes());
            msg.extend(to.to_be_bytes());
            msg.extend(amount.to_be_bytes());
            msg.extend(nonce.to_be_bytes());

            // Sign (sigma protocal)
            let (k, r) = prove_commit(&params.g, &params.p);
            let e = challenge_for_tx(&params.g, &pubkey, &r, &params.p, &msg);
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
            let resp = client
                .post(&url)
                .json(&tx)
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();

            println!("{}", resp);
        }
    }
}
