use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use num_bigint::BigUint;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use zkp::account::Account;
use zkp::dto::AccountSummary;
use zkp::error::RollupError;
use zkp::state::State as RollupState;
use zkp::storage::Storage;
use zkp::transaction::Transaction;

#[derive(Clone)]
struct AppState {
    rollup: Arc<Mutex<RollupState>>,
    storage: Arc<Storage>,
}

#[derive(Serialize)]
struct ParamsResponse {
    p: BigUint,
    g: BigUint,
}

async fn get_params(State(state): State<AppState>) -> Json<ParamsResponse> {
    let s = state.rollup.lock().await;
    Json(ParamsResponse {
        p: s.p.clone(),
        g: s.g.clone(),
    })
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

async fn list_accounts(State(state): State<AppState>) -> Json<Vec<AccountSummary>> {
    let s = state.rollup.lock().await;
    let mut results: Vec<AccountSummary> = s
        .accounts
        .values()
        .map(|a| AccountSummary {
            id: a.id,
            balance: a.balance,
            nonce: a.nonce,
        })
        .collect();
    results.sort_by_key(|a| a.id);
    Json(results)
}

async fn submit_tx(
    State(state): State<AppState>,
    Json(tx): Json<Transaction>,
) -> Result<String, RollupError> {
    let mut s = state.rollup.lock().await;
    s.apply_tx(&tx)?;
    state
        .storage
        .save_accounts(&[&s.accounts[&tx.from], &s.accounts[&tx.to]])
        .unwrap();
    Ok("tx applied".to_string())
}

async fn health() -> &'static str {
    "ok"
}

async fn get_state_root(State(state): State<AppState>) -> String {
    let s = state.rollup.lock().await;
    s.state_root().to_string()
}

async fn get_account(
    Path(id): Path<u32>,
    State(state): State<AppState>,
) -> Result<Json<AccountSummary>, RollupError> {
    let s = state.rollup.lock().await;
    let account = s
        .accounts
        .get(&id)
        .ok_or(RollupError::AccountNotFound { id })?;
    Ok(Json(AccountSummary {
        id: account.id,
        balance: account.balance,
        nonce: account.nonce,
    }))
}

#[tokio::main]
async fn main() {
    let data_path = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string());
    let storage = Storage::open(&data_path).unwrap();

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

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/", get(|| async { "rollup api" }))
        .route("/health", get(health))
        .route("/state-root", get(get_state_root))
        .route("/accounts/{id}", get(get_account))
        .route("/tx", post(submit_tx))
        .route("/accounts", get(list_accounts).post(create_account))
        .route("/params", get(get_params))
        .layer(cors)
        .with_state(app_state);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3000);
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .unwrap();
    println!("Listening on http://0.0.0.0:{port}");
    axum::serve(listener, app).await.unwrap();
}
