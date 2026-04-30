use axum::{
    extract::{Path, State},    
    routing::{get, post},
    Router,
    Json,
    http::StatusCode
};
use num_bigint::BigUint;
use std::sync::Arc;
use tokio::sync::Mutex;
use serde::Deserialize;
use zkp::account::Account;
use zkp::state::State as RollupState;
use zkp::transaction::Transaction;
use zkp::storage::Storage;


#[derive(Clone)]
struct AppState {
    rollup: Arc<Mutex<RollupState>>,
    storage: Arc<Storage>,
}

#[derive(Debug, Deserialize)]
struct CreateAccountRequest {
    id: u32,
    balance: u64,
    pubkey: BigUint,
}

async fn create_account(
    State(state): State<AppState>,
    Json(req): Json<CreateAccountRequest>,
) -> String {
    let account = Account::new(req.id, req.balance, req.pubkey);
    state.storage.save_account(&account).unwrap();
    state.rollup.lock().await.add_account(account);
    format!("account {} created", req.id)
}

async fn submit_tx(
    State(state): State<AppState>,
    Json(tx): Json<Transaction>,
) -> Result<String, (StatusCode, String)> {
    let mut s = state.rollup.lock().await;
    match s.apply_tx(&tx) {
        Ok(()) => {
            state.storage.save_account(&s.accounts[&tx.from]).unwrap();
            state.storage.save_account(&s.accounts[&tx.to]).unwrap();
            Ok("tx applied".to_string())

        }
        Err(e) => Err((StatusCode::BAD_REQUEST, format!("{:?}", e))),
    }
}

async fn health() -> &'static str {
    "ok"
}


async fn get_state_root(State(state): State<AppState>) -> String {
    let s = state.rollup.lock().await;
    s.state_root().to_string()
}


async fn get_balance(
    Path(id): Path<u32>,
    State(state): State<AppState>,
) -> String {
    let s = state.rollup.lock().await;
    match s.accounts.get(&id) {
        Some(a) => a.balance.to_string(),
        None => "account not found".to_string(),
    }
}


#[tokio::main]
async fn main() {
    let storage = Storage::open("./data").unwrap();

    let p = BigUint::from(223u32);
    let g = BigUint::from(4u32);
    let pubkey = BigUint::from(99u32);

    let mut rollup_state = RollupState::new(p, g);

    if storage.load_all_accounts().unwrap().is_empty() {
        let init_accounts = [
            Account::new(1, 100, pubkey.clone()),
            Account::new(2, 50, pubkey.clone()),
            Account::new(3, 200, pubkey.clone()),
            Account::new(4, 0, pubkey.clone()),
        ];
        for a in &init_accounts {
            storage.save_account(a).unwrap();
        }
    }

    for account in storage.load_all_accounts().unwrap() {
        rollup_state.add_account(account);
    }
    let app_state = AppState {
        rollup: Arc::new(Mutex::new(rollup_state)),
        storage: Arc::new(storage),
    };

    let app = Router::new()
        .route("/", get(|| async { "rollup api" }))
        .route("/health", get(health))
        .route("/state-root", get(get_state_root))
        .route("/balance/{id}", get(get_balance))
        .route("/tx", post(submit_tx))
        .route("/accounts", post(create_account))
        .with_state(app_state);
    
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Listening on http://0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}