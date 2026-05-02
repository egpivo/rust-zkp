use axum::{
    Json, Router,
    extract::{Path, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
};
use num_bigint::BigUint;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tokio::time::{Duration, interval};
use tower_http::cors::{Any, CorsLayer};
use tracing::{info, instrument, warn};
use tracing_subscriber::EnvFilter;
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
    mempool_tx: mpsc::Sender<Transaction>,
}

#[derive(Serialize)]
struct ParamsResponse {
    #[serde(with = "zkp::serde_helpers::biguint_string")]
    p: BigUint,
    #[serde(with = "zkp::serde_helpers::biguint_string")]
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
    #[serde(with = "zkp::serde_helpers::biguint_string")]
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

#[instrument(
    skip_all,
    fields(from = tx.from, to = tx.to, amount = tx.amount, nonce = tx.nonce)
)]
async fn submit_tx(
    State(state): State<AppState>,
    Json(tx): Json<Transaction>,
) -> Result<(StatusCode, String), RollupError> {
    state.mempool_tx.send(tx).await.map_err(|_| {
        warn!("mempool full");
        RollupError::StateRootMismatch
    })?;
    info!("queued");
    Ok((StatusCode::ACCEPTED, "tx queued".to_string()))
}

async fn get_mempool(State(state): State<AppState>) -> Json<serde_json::Value> {
    let capacity = state.mempool_tx.capacity();
    let max = state.mempool_tx.max_capacity();
    Json(serde_json::json!({
        "available_slots": capacity,
        "max_capacity": max,
        "pending": max - capacity,
    }))
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

async fn require_api_key(
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, (StatusCode, &'static str)> {
    let expected = std::env::var("API_KEY").unwrap_or_default();

    // No API_KEY configured -> skip auth (dev mode)
    if expected.is_empty() {
        return Ok(next.run(request).await);
    }

    let provided = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if provided == expected {
        Ok(next.run(request).await)
    } else {
        Err((
            StatusCode::UNAUTHORIZED,
            "missing or invalid x-api-key header",
        ))
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let data_path = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string());
    let storage = Storage::open(&data_path).unwrap();
    let (mempool_tx, mempool_rx) = mpsc::channel::<Transaction>(1000);

    let p = BigUint::from(223u32);
    let g = BigUint::from(4u32);

    // Seed pubkey derived from a known secret so the playground demos
    // can sign transactions for the seeded accounts without prior registration.
    // DEMO ONLY: in real systems each account has its own secret.
    let demo_secret = BigUint::from(12345u32);
    let demo_pubkey = g.modpow(&demo_secret, &p);

    let mut rollup_state = RollupState::new(p, g);

    if storage.load_all_accounts().unwrap().is_empty() {
        let init_accounts = [
            Account::new(1, 100, demo_pubkey.clone()),
            Account::new(2, 50, demo_pubkey.clone()),
            Account::new(3, 200, demo_pubkey.clone()),
            Account::new(4, 0, demo_pubkey.clone()),
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
        mempool_tx,
    };
    let bg_state = app_state.clone();
    let mut bg_rx = mempool_rx;
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(5));
        loop {
            tick.tick().await;

            // Drain non-blocking
            let mut txs = Vec::new();
            while let Ok(tx) = bg_rx.try_recv() {
                txs.push(tx);
            }

            if txs.is_empty() {
                continue;
            }

            let count = txs.len();
            let mut applied = 0;
            let mut s = bg_state.rollup.lock().await;
            for tx in &txs {
                if s.apply_tx(tx).is_ok() {
                    let from = s.accounts[&tx.from].clone();
                    let to = s.accounts[&tx.to].clone();
                    bg_state.storage.save_accounts(&[&from, &to]).ok();
                    applied += 1;
                }
            }
            drop(s);

            info!(applied, count, "batch applied");
        }
    });

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
        .route("/accounts", get(list_accounts))
        .route(
            "/accounts",
            post(create_account).layer(middleware::from_fn(require_api_key)),
        )
        .route("/params", get(get_params))
        .route("/mempool", get(get_mempool))
        .layer(cors)
        .with_state(app_state);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3000);
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .unwrap();
    info!(port, "rollup server listening");
    axum::serve(listener, app).await.unwrap();
}
