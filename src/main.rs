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
use zkp::account::Account;
use zkp::state::State as RollupState;
use zkp::transaction::Transaction;
use serde::Deserialize;


type SharedState = Arc<Mutex<RollupState>>;

#[derive(Debug, Deserialize)]
struct CreateAccountRequest {
    id: u32,
    balance: u64,
    pubkey: BigUint,
}

async fn create_account(
    State(state): State<SharedState>,
    Json(req): Json<CreateAccountRequest>,
) -> String {
    let mut s = state.lock().await;
    s.add_account(Account::new(req.id, req.balance, req.pubkey));
    format!("account {} created", req.id)
}

async fn health() -> &'static str {
    "ok"
}


async fn get_state_root(State(state): State<SharedState>) -> String {
    let s = state.lock().await;
    s.state_root().to_string()
}


async fn get_balance(
    Path(id): Path<u32>,
    State(state): State<SharedState>,
) -> String {
    let s = state.lock().await;
    match s.accounts.get(&id) {
        Some(a) => a.balance.to_string(),
        None => "account not found".to_string(),
    }
}


async fn submit_tx(
    State(state): State<SharedState>,
    Json(tx): Json<Transaction>,
) -> Result<String, (StatusCode, String)>{
    let mut s = state.lock().await;
    match s.apply_tx(&tx) {
        Ok(()) => Ok("tx applied".to_string()),
        Err(e) => Err((StatusCode::BAD_REQUEST, format!("{:?}", e))),
    }
}


#[tokio::main]
async fn main() {
    let p = BigUint::from(223u32);
    let g = BigUint::from(4u32);
    let pubkey = BigUint::from(99u32);

    let mut rollup_state = RollupState::new(p, g);
    rollup_state.add_account(Account::new(1, 100, pubkey.clone()));
    rollup_state.add_account(Account::new(2, 50, pubkey.clone()));
    rollup_state.add_account(Account::new(3, 200, pubkey.clone()));
    rollup_state.add_account(Account::new(4, 0, pubkey.clone()));

    let shared = Arc::new(Mutex::new(rollup_state));

    let app = Router::new()
        .route("/", get(|| async { "rollup api" }))
        .route("/health", get(health))
        .route("/state-root", get(get_state_root))
        .route("/balance/{id}", get(get_balance))
        .route("/tx", post(submit_tx))
        .route("/accounts", post(create_account))
        .with_state(shared);
    
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Listening on http://0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}