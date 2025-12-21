use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use ethers::types::U256;
use serde::{Deserialize, Serialize};
use sqlx::Row;

const DEFAULT_PAGE_LIMIT: i64 = 50;
const MAX_PAGE_LIMIT: i64 = 100;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/raffles", get(get_raffles))
        .route("/raffles/:raffle_id", get(get_raffle))
        .route("/raffles/:raffle_id/purchases", get(get_purchases))
        .route("/raffles/:raffle_id/proof", get(get_proof))
}

#[derive(Deserialize)]
struct RafflesQuery {
    limit: Option<i64>,
    offset: Option<i64>,
    status: Option<String>,
}

#[derive(Deserialize)]
struct PaginationQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Serialize)]
struct RaffleSummary {
    raffle_id: i64,
    raffle_address: String,
    status: String,
    end_time: Option<DateTime<Utc>>,
    ticket_price: String,
    total_tickets: i64,
    pot: String,
    winner: Option<String>,
}

#[derive(Serialize)]
struct RaffleDetails {
    raffle_id: i64,
    raffle_address: String,
    creator: String,
    end_time: Option<DateTime<Utc>>,
    ticket_price: String,
    max_tickets: i64,
    fee_bps: i64,
    fee_recipient: String,
    status: String,
    total_tickets: i64,
    pot: String,
    request_id: Option<String>,
    request_tx: Option<String>,
    randomness: Option<String>,
    randomness_tx: Option<String>,
    winning_index: Option<i64>,
    winner: Option<String>,
    finalized_tx: Option<String>,
}

#[derive(Serialize)]
struct PurchaseRange {
    buyer: String,
    start_index: i64,
    end_index: i64,
    count: i64,
    amount: String,
    tx_hash: String,
    log_index: i64,
    block_number: i64,
    created_at: DateTime<Utc>,
}

#[derive(Serialize)]
struct WinningRange {
    buyer: String,
    start_index: i64,
    end_index: i64,
}

#[derive(Serialize)]
struct TxLinks {
    request_tx: Option<String>,
    request_url: Option<String>,
    randomness_tx: Option<String>,
    randomness_url: Option<String>,
    finalized_tx: Option<String>,
    finalized_url: Option<String>,
}

#[derive(Serialize)]
struct ProofResponse {
    raffle_id: i64,
    request_id: Option<String>,
    randomness: Option<String>,
    total_tickets: i64,
    winning_index: Option<i64>,
    winner: Option<String>,
    winning_range: Option<WinningRange>,
    txs: TxLinks,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let body = Json(ErrorResponse { error: self.message });
        (self.status, body).into_response()
    }
}

async fn get_raffles(
    State(state): State<AppState>,
    Query(params): Query<RafflesQuery>,
) -> Result<Json<Vec<RaffleSummary>>, ApiError> {
    let limit = normalize_limit(params.limit)?;
    let offset = normalize_offset(params.offset)?;
    let raffle_rows = if let Some(status) = params.status {
        sqlx::query(
            "SELECT raffle_id, raffle_address, status, end_time,
                ticket_price::text AS ticket_price,
                total_tickets, pot::text AS pot, winner
             FROM raffles
             WHERE status = $1
             ORDER BY raffle_id DESC
             LIMIT $2 OFFSET $3",
        )
        .bind(status)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
        .map_err(|err| ApiError::internal(err.to_string()))?
    } else {
        sqlx::query(
            "SELECT raffle_id, raffle_address, status, end_time,
                ticket_price::text AS ticket_price,
                total_tickets, pot::text AS pot, winner
             FROM raffles
             ORDER BY raffle_id DESC
             LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
        .map_err(|err| ApiError::internal(err.to_string()))?
    };

    let mut raffles = Vec::with_capacity(raffle_rows.len());
    for row in raffle_rows {
        raffles.push(RaffleSummary {
            raffle_id: row.try_get("raffle_id").map_err(map_row_error)?,
            raffle_address: row.try_get("raffle_address").map_err(map_row_error)?,
            status: row.try_get("status").map_err(map_row_error)?,
            end_time: row.try_get("end_time").map_err(map_row_error)?,
            ticket_price: row.try_get("ticket_price").map_err(map_row_error)?,
            total_tickets: row.try_get("total_tickets").map_err(map_row_error)?,
            pot: row.try_get("pot").map_err(map_row_error)?,
            winner: row.try_get("winner").map_err(map_row_error)?,
        });
    }

    Ok(Json(raffles))
}

async fn get_raffle(
    State(state): State<AppState>,
    Path(raffle_id): Path<i64>,
) -> Result<Json<RaffleDetails>, ApiError> {
    let row = sqlx::query(
        "SELECT raffle_id, raffle_address, creator, end_time,
            ticket_price::text AS ticket_price,
            max_tickets, fee_bps, fee_recipient, status,
            total_tickets, pot::text AS pot, request_id, request_tx,
            randomness, randomness_tx, winning_index, winner, finalized_tx
         FROM raffles
         WHERE raffle_id = $1",
    )
    .bind(raffle_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| ApiError::internal(err.to_string()))?;

    let Some(row) = row else {
        return Err(ApiError::not_found("raffle not found"));
    };

    Ok(Json(RaffleDetails {
        raffle_id: row.try_get("raffle_id").map_err(map_row_error)?,
        raffle_address: row.try_get("raffle_address").map_err(map_row_error)?,
        creator: row.try_get("creator").map_err(map_row_error)?,
        end_time: row.try_get("end_time").map_err(map_row_error)?,
        ticket_price: row.try_get("ticket_price").map_err(map_row_error)?,
        max_tickets: row.try_get("max_tickets").map_err(map_row_error)?,
        fee_bps: row.try_get("fee_bps").map_err(map_row_error)?,
        fee_recipient: row.try_get("fee_recipient").map_err(map_row_error)?,
        status: row.try_get("status").map_err(map_row_error)?,
        total_tickets: row.try_get("total_tickets").map_err(map_row_error)?,
        pot: row.try_get("pot").map_err(map_row_error)?,
        request_id: row.try_get("request_id").map_err(map_row_error)?,
        request_tx: row.try_get("request_tx").map_err(map_row_error)?,
        randomness: row.try_get("randomness").map_err(map_row_error)?,
        randomness_tx: row.try_get("randomness_tx").map_err(map_row_error)?,
        winning_index: row.try_get("winning_index").map_err(map_row_error)?,
        winner: row.try_get("winner").map_err(map_row_error)?,
        finalized_tx: row.try_get("finalized_tx").map_err(map_row_error)?,
    }))
}

async fn get_purchases(
    State(state): State<AppState>,
    Path(raffle_id): Path<i64>,
    Query(params): Query<PaginationQuery>,
) -> Result<Json<Vec<PurchaseRange>>, ApiError> {
    let limit = normalize_limit(params.limit)?;
    let offset = normalize_offset(params.offset)?;
    let purchase_rows = sqlx::query(
        "SELECT buyer, start_index, end_index, count,
            amount::text AS amount, tx_hash, log_index, block_number, created_at
         FROM purchases
         WHERE raffle_id = $1
         ORDER BY id ASC
         LIMIT $2 OFFSET $3",
    )
    .bind(raffle_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|err| ApiError::internal(err.to_string()))?;

    let mut purchases = Vec::with_capacity(purchase_rows.len());
    for row in purchase_rows {
        purchases.push(PurchaseRange {
            buyer: row.try_get("buyer").map_err(map_row_error)?,
            start_index: row.try_get("start_index").map_err(map_row_error)?,
            end_index: row.try_get("end_index").map_err(map_row_error)?,
            count: row.try_get("count").map_err(map_row_error)?,
            amount: row.try_get("amount").map_err(map_row_error)?,
            tx_hash: row.try_get("tx_hash").map_err(map_row_error)?,
            log_index: row.try_get("log_index").map_err(map_row_error)?,
            block_number: row.try_get("block_number").map_err(map_row_error)?,
            created_at: row.try_get("created_at").map_err(map_row_error)?,
        });
    }

    Ok(Json(purchases))
}

async fn get_proof(
    State(state): State<AppState>,
    Path(raffle_id): Path<i64>,
) -> Result<Json<ProofResponse>, ApiError> {
    let raffle_row = sqlx::query(
        "SELECT raffle_id, request_id, request_tx, randomness, randomness_tx,
            winning_index, winner, total_tickets, finalized_tx
         FROM raffles
         WHERE raffle_id = $1",
    )
    .bind(raffle_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| ApiError::internal(err.to_string()))?;

    let Some(row) = raffle_row else {
        return Err(ApiError::not_found("raffle not found"));
    };

    let request_id: Option<String> = row.try_get("request_id").map_err(map_row_error)?;
    let request_tx: Option<String> = row.try_get("request_tx").map_err(map_row_error)?;
    let randomness: Option<String> = row.try_get("randomness").map_err(map_row_error)?;
    let randomness_tx: Option<String> = row.try_get("randomness_tx").map_err(map_row_error)?;
    let winner: Option<String> = row.try_get("winner").map_err(map_row_error)?;
    let finalized_tx: Option<String> = row.try_get("finalized_tx").map_err(map_row_error)?;
    let total_tickets: i64 = row.try_get("total_tickets").map_err(map_row_error)?;
    let mut winning_index: Option<i64> = row.try_get("winning_index").map_err(map_row_error)?;

    // If the winning index was not stored, recompute it from randomness.
    if winning_index.is_none() {
        if let Some(randomness) = randomness.as_ref() {
            if total_tickets > 0 {
                if let Ok(rand) = U256::from_dec_str(randomness) {
                    let idx = (rand % U256::from(total_tickets as u64)).as_u64() as i64;
                    winning_index = Some(idx);
                }
            }
        }
    }

    let winning_range = if let Some(index) = winning_index {
        let row = sqlx::query(
            "SELECT buyer, start_index, end_index
             FROM purchases
             WHERE raffle_id = $1 AND start_index <= $2 AND end_index >= $2
             ORDER BY id ASC
             LIMIT 1",
        )
        .bind(raffle_id)
        .bind(index)
        .fetch_optional(&state.db)
        .await
        .map_err(|err| ApiError::internal(err.to_string()))?;

        if let Some(row) = row {
            Some(WinningRange {
                buyer: row.try_get("buyer").map_err(map_row_error)?,
                start_index: row.try_get("start_index").map_err(map_row_error)?,
                end_index: row.try_get("end_index").map_err(map_row_error)?,
            })
        } else {
            None
        }
    } else {
        None
    };

    let txs = TxLinks {
        request_tx: request_tx.clone(),
        request_url: tx_url(&state.config.explorer_base_url, &request_tx),
        randomness_tx: randomness_tx.clone(),
        randomness_url: tx_url(&state.config.explorer_base_url, &randomness_tx),
        finalized_tx: finalized_tx.clone(),
        finalized_url: tx_url(&state.config.explorer_base_url, &finalized_tx),
    };

    Ok(Json(ProofResponse {
        raffle_id: row.try_get("raffle_id").map_err(map_row_error)?,
        request_id,
        randomness,
        total_tickets,
        winning_index,
        winner,
        winning_range,
        txs,
    }))
}

fn trim_slash(base: &str) -> &str {
    base.trim_end_matches('/')
}

fn normalize_limit(limit: Option<i64>) -> Result<i64, ApiError> {
    let limit = limit.unwrap_or(DEFAULT_PAGE_LIMIT);
    if limit <= 0 {
        return Err(ApiError::bad_request("limit must be positive"));
    }
    Ok(limit.min(MAX_PAGE_LIMIT))
}

fn normalize_offset(offset: Option<i64>) -> Result<i64, ApiError> {
    let offset = offset.unwrap_or(0);
    if offset < 0 {
        return Err(ApiError::bad_request("offset must be >= 0"));
    }
    Ok(offset)
}

fn map_row_error(err: sqlx::Error) -> ApiError {
    ApiError::internal(err.to_string())
}

fn tx_url(explorer_base_url: &str, tx_hash: &Option<String>) -> Option<String> {
    tx_hash
        .as_ref()
        .map(|hash| format!("{}/tx/{}", trim_slash(explorer_base_url), hash))
}
