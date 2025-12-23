//! REST API handlers for Ticket Arcade
//!
//! Provides endpoints for querying raffle data indexed from the blockchain.
//! All data is read-only and cached from on-chain events.
//!
//! # Endpoints
//! - `GET /v1/raffles` - List raffles with pagination and optional status filter
//! - `GET /v1/raffles/:raffle_id` - Get raffle details
//! - `GET /v1/raffles/:raffle_id/purchases` - Get ticket purchase ranges
//! - `GET /v1/raffles/:raffle_id/proof` - Get verification proof data
//!
//! # Security Considerations
//! - All queries use parameterized SQL (no injection risk)
//! - Pagination is enforced with maximum limits
//! - Error messages don't expose internal details

use crate::state::AppState;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use chrono::{DateTime, Utc};
use ethers::types::U256;
use serde::{Deserialize, Serialize};
use sqlx::Row;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Default number of items per page
const DEFAULT_PAGE_LIMIT: i64 = 50;
/// Maximum allowed items per page (prevents DoS via large queries)
const MAX_PAGE_LIMIT: i64 = 100;

// ============================================================================
// ROUTER
// ============================================================================

/// Creates the v1 API router with all raffle endpoints
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/raffles", get(list_raffles))
        .route("/raffles/:raffle_id", get(get_raffle_by_id))
        .route("/raffles/:raffle_id/purchases", get(list_purchases))
        .route("/raffles/:raffle_id/proof", get(get_raffle_proof))
}

// ============================================================================
// REQUEST/RESPONSE TYPES
// ============================================================================

/// Query parameters for listing raffles
#[derive(Deserialize)]
struct ListRafflesQuery {
    limit: Option<i64>,
    offset: Option<i64>,
    /// Filter by status: ACTIVE, CLOSED, RANDOM_REQUESTED, RANDOM_FULFILLED, FINALIZED, REFUNDING
    status: Option<String>,
}

/// Query parameters for paginated lists
#[derive(Deserialize)]
struct PaginationQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

/// Summary view of a raffle for list endpoints
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
        let body = Json(ErrorResponse {
            error: self.message,
        });
        (self.status, body).into_response()
    }
}

// ============================================================================
// HANDLERS
// ============================================================================

/// GET /v1/raffles - List raffles with optional status filter
async fn list_raffles(
    State(state): State<AppState>,
    Query(params): Query<ListRafflesQuery>,
) -> Result<Json<Vec<RaffleSummary>>, ApiError> {
    let limit = normalize_limit(params.limit)?;
    let offset = normalize_offset(params.offset)?;

    // Use parameterized query - safe from SQL injection
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
        .map_err(db_error_to_api_error)?
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
        .map_err(db_error_to_api_error)?
    };

    let mut raffles = Vec::with_capacity(raffle_rows.len());
    for row in raffle_rows {
        raffles.push(RaffleSummary {
            raffle_id: row.try_get("raffle_id").map_err(row_error_to_api_error)?,
            raffle_address: row
                .try_get("raffle_address")
                .map_err(row_error_to_api_error)?,
            status: row.try_get("status").map_err(row_error_to_api_error)?,
            end_time: row.try_get("end_time").map_err(row_error_to_api_error)?,
            ticket_price: row
                .try_get("ticket_price")
                .map_err(row_error_to_api_error)?,
            total_tickets: row
                .try_get("total_tickets")
                .map_err(row_error_to_api_error)?,
            pot: row.try_get("pot").map_err(row_error_to_api_error)?,
            winner: row.try_get("winner").map_err(row_error_to_api_error)?,
        });
    }

    Ok(Json(raffles))
}

/// GET /v1/raffles/:raffle_id - Get raffle details by ID
async fn get_raffle_by_id(
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
    .map_err(db_error_to_api_error)?;

    let Some(row) = row else {
        return Err(ApiError::not_found("raffle not found"));
    };

    Ok(Json(RaffleDetails {
        raffle_id: row.try_get("raffle_id").map_err(row_error_to_api_error)?,
        raffle_address: row
            .try_get("raffle_address")
            .map_err(row_error_to_api_error)?,
        creator: row.try_get("creator").map_err(row_error_to_api_error)?,
        end_time: row.try_get("end_time").map_err(row_error_to_api_error)?,
        ticket_price: row
            .try_get("ticket_price")
            .map_err(row_error_to_api_error)?,
        max_tickets: row.try_get("max_tickets").map_err(row_error_to_api_error)?,
        fee_bps: row.try_get("fee_bps").map_err(row_error_to_api_error)?,
        fee_recipient: row
            .try_get("fee_recipient")
            .map_err(row_error_to_api_error)?,
        status: row.try_get("status").map_err(row_error_to_api_error)?,
        total_tickets: row
            .try_get("total_tickets")
            .map_err(row_error_to_api_error)?,
        pot: row.try_get("pot").map_err(row_error_to_api_error)?,
        request_id: row.try_get("request_id").map_err(row_error_to_api_error)?,
        request_tx: row.try_get("request_tx").map_err(row_error_to_api_error)?,
        randomness: row.try_get("randomness").map_err(row_error_to_api_error)?,
        randomness_tx: row
            .try_get("randomness_tx")
            .map_err(row_error_to_api_error)?,
        winning_index: row
            .try_get("winning_index")
            .map_err(row_error_to_api_error)?,
        winner: row.try_get("winner").map_err(row_error_to_api_error)?,
        finalized_tx: row
            .try_get("finalized_tx")
            .map_err(row_error_to_api_error)?,
    }))
}

/// GET /v1/raffles/:raffle_id/purchases - List ticket purchases for a raffle
async fn list_purchases(
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
    .map_err(db_error_to_api_error)?;

    let mut purchases = Vec::with_capacity(purchase_rows.len());
    for row in purchase_rows {
        purchases.push(PurchaseRange {
            buyer: row.try_get("buyer").map_err(row_error_to_api_error)?,
            start_index: row.try_get("start_index").map_err(row_error_to_api_error)?,
            end_index: row.try_get("end_index").map_err(row_error_to_api_error)?,
            count: row.try_get("count").map_err(row_error_to_api_error)?,
            amount: row.try_get("amount").map_err(row_error_to_api_error)?,
            tx_hash: row.try_get("tx_hash").map_err(row_error_to_api_error)?,
            log_index: row.try_get("log_index").map_err(row_error_to_api_error)?,
            block_number: row
                .try_get("block_number")
                .map_err(row_error_to_api_error)?,
            created_at: row.try_get("created_at").map_err(row_error_to_api_error)?,
        });
    }

    Ok(Json(purchases))
}

/// GET /v1/raffles/:raffle_id/proof - Get verification proof for a raffle
///
/// Returns randomness, winning index, winner address, and relevant transaction links
/// for client-side verification of fair winner selection.
async fn get_raffle_proof(
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
    .map_err(db_error_to_api_error)?;

    let Some(row) = raffle_row else {
        return Err(ApiError::not_found("raffle not found"));
    };

    let request_id: Option<String> = row.try_get("request_id").map_err(row_error_to_api_error)?;
    let request_tx: Option<String> = row.try_get("request_tx").map_err(row_error_to_api_error)?;
    let randomness: Option<String> = row.try_get("randomness").map_err(row_error_to_api_error)?;
    let randomness_tx: Option<String> = row
        .try_get("randomness_tx")
        .map_err(row_error_to_api_error)?;
    let winner: Option<String> = row.try_get("winner").map_err(row_error_to_api_error)?;
    let finalized_tx: Option<String> = row
        .try_get("finalized_tx")
        .map_err(row_error_to_api_error)?;
    let total_tickets: i64 = row
        .try_get("total_tickets")
        .map_err(row_error_to_api_error)?;
    let mut winning_index: Option<i64> = row
        .try_get("winning_index")
        .map_err(row_error_to_api_error)?;

    // If the winning index was not stored, recompute it from randomness.
    // This allows clients to verify: winningIndex = randomness % totalTickets
    if winning_index.is_none()
        && let Some(ref randomness_str) = randomness
        && total_tickets > 0
        && let Ok(rand) = U256::from_dec_str(randomness_str)
    {
        let idx = (rand % U256::from(total_tickets as u64)).as_u64() as i64;
        winning_index = Some(idx);
    }

    // Look up the winning ticket range
    let winning_range = if let Some(index) = winning_index {
        let range_row = sqlx::query(
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
        .map_err(db_error_to_api_error)?;

        range_row.map(|r| WinningRange {
            buyer: r.try_get("buyer").unwrap_or_default(),
            start_index: r.try_get("start_index").unwrap_or_default(),
            end_index: r.try_get("end_index").unwrap_or_default(),
        })
    } else {
        None
    };

    let txs = TxLinks {
        request_tx: request_tx.clone(),
        request_url: build_tx_url(&state.config.explorer_base_url, &request_tx),
        randomness_tx: randomness_tx.clone(),
        randomness_url: build_tx_url(&state.config.explorer_base_url, &randomness_tx),
        finalized_tx: finalized_tx.clone(),
        finalized_url: build_tx_url(&state.config.explorer_base_url, &finalized_tx),
    };

    Ok(Json(ProofResponse {
        raffle_id: row.try_get("raffle_id").map_err(row_error_to_api_error)?,
        request_id,
        randomness,
        total_tickets,
        winning_index,
        winner,
        winning_range,
        txs,
    }))
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Normalizes pagination limit with bounds checking
fn normalize_limit(limit: Option<i64>) -> Result<i64, ApiError> {
    let limit = limit.unwrap_or(DEFAULT_PAGE_LIMIT);
    if limit <= 0 {
        return Err(ApiError::bad_request("limit must be positive"));
    }
    Ok(limit.min(MAX_PAGE_LIMIT))
}

/// Normalizes pagination offset with bounds checking
fn normalize_offset(offset: Option<i64>) -> Result<i64, ApiError> {
    let offset = offset.unwrap_or(0);
    if offset < 0 {
        return Err(ApiError::bad_request("offset must be >= 0"));
    }
    Ok(offset)
}

/// Converts database error to API error without exposing internal details
fn db_error_to_api_error(err: sqlx::Error) -> ApiError {
    // Log the actual error for debugging, but don't expose to client
    tracing::error!(error = %err, "database error");
    ApiError::internal("database error")
}

/// Converts row extraction error to API error
fn row_error_to_api_error(err: sqlx::Error) -> ApiError {
    tracing::error!(error = %err, "row extraction error");
    ApiError::internal("data extraction error")
}

/// Builds a block explorer URL for a transaction hash
fn build_tx_url(explorer_base_url: &str, tx_hash: &Option<String>) -> Option<String> {
    tx_hash.as_ref().map(|hash| {
        let base = explorer_base_url.trim_end_matches('/');
        format!("{}/tx/{}", base, hash)
    })
}
