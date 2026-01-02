//! On-chain event indexer for Ticket Arcade
//!
//! This module polls the Arc L1 RPC for contract events and stores them in PostgreSQL.
//! It is designed to be idempotent: restarting the indexer will not duplicate events
//! thanks to unique constraints on (tx_hash, log_index).
//!
//! # Security Considerations
//! - All RPC calls have timeouts to prevent hanging
//! - Database operations use parameterized queries (no SQL injection)
//! - Errors are logged without exposing sensitive data
//! - Idempotent inserts prevent duplicate event processing

use crate::config::AppConfig;
use anyhow::{Context, anyhow};
use chrono::{DateTime, Utc};
use ethers::abi::{Abi, Event, RawLog, Token};
use ethers::providers::{Http, Middleware, Provider};
use ethers::types::{Address, Filter, H256, Log, U256};
use sqlx::{PgPool, Row};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Path to RaffleFactory ABI artifact (relative to crate root)
const FACTORY_ARTIFACT_PATH: &str =
    "../contracts/artifacts/contracts/RaffleFactory.sol/RaffleFactory.json";
/// Path to Raffle ABI artifact (relative to crate root)
const RAFFLE_ARTIFACT_PATH: &str = "../contracts/artifacts/contracts/Raffle.sol/Raffle.json";
/// Path to DrandRandomnessProvider ABI artifact (relative to crate root)
const DRAND_PROVIDER_ARTIFACT_PATH: &str =
    "../contracts/artifacts/contracts/DrandRandomnessProvider.sol/DrandRandomnessProvider.json";

/// Timeout for individual RPC calls (prevents hanging on unresponsive nodes)
const RPC_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum number of addresses to query in a single filter to prevent DoS
const MAX_ADDRESSES_PER_QUERY: usize = 100;

/// Backoff sleep duration when RPC errors occur
const ERROR_BACKOFF: Duration = Duration::from_secs(5);

// ============================================================================
// TYPES
// ============================================================================

/// Enumeration of all indexed event types
#[derive(Clone, Copy, Debug)]
enum EventKind {
    // Factory events
    RaffleCreated,
    // Raffle events
    TicketsBought,
    RaffleClosed,
    RandomnessRequested,
    RandomnessFulfilled,
    WinnerSelected,
    RefundClaimed,
    KeeperUpdated,
    RefundsStarted,
    PayoutsCompleted,
    // DrandRandomnessProvider events
    ProviderRandomnessRequested,
    ProviderRandomnessDelivered,
}

/// Event definition combining kind with ABI for decoding
#[derive(Clone, Debug)]
struct EventDef {
    kind: EventKind,
    event: Event,
}

// ============================================================================
// MAIN ENTRY POINT
// ============================================================================

/// Main indexer loop - runs indefinitely, polling for new blocks
///
/// # Arguments
/// * `db_pool` - PostgreSQL connection pool
/// * `config` - Application configuration
///
/// # Errors
/// Returns error only for unrecoverable issues (ABI load failure, chain ID mismatch).
/// Transient RPC/DB errors trigger backoff and retry.
pub async fn run(db_pool: PgPool, config: AppConfig) -> anyhow::Result<()> {
    let provider = Provider::<Http>::try_from(config.rpc_url.as_str())?
        .interval(Duration::from_millis(config.indexer_poll_interval_ms));

    // Verify chain ID with timeout (security: prevent wrong-chain indexing)
    let rpc_chain_id = tokio::time::timeout(RPC_TIMEOUT, provider.get_chainid())
        .await
        .context("chain ID request timed out")?
        .context("failed to get chain ID")?
        .as_u64();

    if rpc_chain_id != config.chain_id {
        return Err(anyhow!(
            "RPC chain ID {} does not match configured chain ID {}",
            rpc_chain_id,
            config.chain_id
        ));
    }

    // Load ABIs from artifact files
    let factory_abi =
        load_abi(FACTORY_ARTIFACT_PATH).context("failed to load RaffleFactory ABI")?;
    let raffle_abi = load_abi(RAFFLE_ARTIFACT_PATH).context("failed to load Raffle ABI")?;
    let provider_abi = load_abi(DRAND_PROVIDER_ARTIFACT_PATH).ok(); // Optional - may not exist yet

    let events_by_signature = build_event_map(&factory_abi, &raffle_abi, provider_abi.as_ref())?;
    let factory_address = Address::from_str(&config.raffle_factory_address)
        .context("invalid factory address format")?;

    // Parse optional randomness provider address
    let provider_address = config
        .randomness_provider_address
        .as_ref()
        .and_then(|addr| Address::from_str(addr).ok());

    tracing::info!(
        start_block = config.start_block,
        batch_size = config.indexer_batch_size,
        factory = %factory_address,
        provider = ?provider_address,
        "indexer started"
    );

    // Main polling loop with error recovery
    loop {
        match run_indexing_cycle(
            &db_pool,
            &config,
            &provider,
            &events_by_signature,
            factory_address,
            provider_address,
        )
        .await
        {
            Ok(()) => {}
            Err(err) => {
                // Log without exposing sensitive details, then backoff
                tracing::error!(error = %err, "indexing cycle failed, retrying after backoff");
                tokio::time::sleep(ERROR_BACKOFF).await;
            }
        }
    }
}

/// Executes a single indexing cycle (poll and process one batch)
async fn run_indexing_cycle(
    db_pool: &PgPool,
    config: &AppConfig,
    provider: &Provider<Http>,
    events_by_signature: &HashMap<H256, EventDef>,
    factory_address: Address,
    provider_address: Option<Address>,
) -> anyhow::Result<()> {
    // Get latest block with timeout
    let latest = tokio::time::timeout(RPC_TIMEOUT, provider.get_block_number())
        .await
        .context("get_block_number timed out")?
        .context("failed to get latest block number")?
        .as_u64();

    let last_processed = get_last_processed_block(db_pool).await?;
    let mut from_block = if last_processed == 0 {
        config.start_block
    } else {
        last_processed.saturating_add(1)
    };
    from_block = from_block.max(config.start_block);

    // Nothing new to process - sleep and return
    if from_block > latest {
        tokio::time::sleep(Duration::from_millis(config.indexer_poll_interval_ms)).await;
        return Ok(());
    }

    // Calculate batch end (use saturating math to prevent overflow)
    let to_block = from_block
        .saturating_add(config.indexer_batch_size.saturating_sub(1))
        .min(latest);
    tracing::info!(from_block, to_block, "processing block range");

    // 1. Fetch and process factory events (RaffleCreated)
    let factory_logs =
        fetch_logs_with_timeout(provider, vec![factory_address], from_block, to_block)
            .await
            .context("failed to fetch factory logs")?;

    for log_entry in &factory_logs {
        if let Err(err) = process_log(db_pool, events_by_signature, log_entry).await {
            tracing::warn!(
                tx_hash = ?log_entry.transaction_hash,
                error = %err,
                "failed to process factory log, skipping"
            );
        }
    }

    // 2. Fetch and process randomness provider events (if configured)
    if let Some(prov_addr) = provider_address {
        let provider_logs =
            fetch_logs_with_timeout(provider, vec![prov_addr], from_block, to_block)
                .await
                .context("failed to fetch provider logs")?;

        for log_entry in &provider_logs {
            if let Err(err) = process_log(db_pool, events_by_signature, log_entry).await {
                tracing::warn!(
                    tx_hash = ?log_entry.transaction_hash,
                    error = %err,
                    "failed to process provider log, skipping"
                );
            }
        }
    }

    // 3. Load known raffle addresses and fetch their events
    let raffle_addresses = load_raffle_addresses(db_pool).await?;
    if !raffle_addresses.is_empty() {
        // Process in chunks to prevent DoS via unbounded queries
        for chunk in raffle_addresses.chunks(MAX_ADDRESSES_PER_QUERY) {
            let raffle_logs =
                fetch_logs_with_timeout(provider, chunk.to_vec(), from_block, to_block)
                    .await
                    .context("failed to fetch raffle logs")?;

            for log_entry in &raffle_logs {
                if let Err(err) = process_log(db_pool, events_by_signature, log_entry).await {
                    tracing::warn!(
                        tx_hash = ?log_entry.transaction_hash,
                        error = %err,
                        "failed to process raffle log, skipping"
                    );
                }
            }
        }
    }

    // 4. Update last processed block
    set_last_processed_block(db_pool, to_block).await?;
    Ok(())
}

/// Fetches logs with timeout and deterministic ordering
async fn fetch_logs_with_timeout(
    provider: &Provider<Http>,
    addresses: Vec<Address>,
    from_block: u64,
    to_block: u64,
) -> anyhow::Result<Vec<Log>> {
    let filter = Filter::new()
        .address(addresses)
        .from_block(from_block)
        .to_block(to_block);

    let mut logs = tokio::time::timeout(RPC_TIMEOUT, provider.get_logs(&filter))
        .await
        .context("get_logs timed out")?
        .context("failed to fetch logs")?;

    // Sort for deterministic processing order (block number, then log index)
    logs.sort_by(|a, b| {
        let block_cmp = a
            .block_number
            .unwrap_or_default()
            .cmp(&b.block_number.unwrap_or_default());
        match block_cmp {
            Ordering::Equal => a
                .log_index
                .unwrap_or_default()
                .cmp(&b.log_index.unwrap_or_default()),
            other => other,
        }
    });

    Ok(logs)
}

/// Loads an ABI from a Hardhat artifact JSON file
fn load_abi(relative_path: &str) -> anyhow::Result<Abi> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read ABI file: {}", path.display()))?;

    let json: serde_json::Value =
        serde_json::from_str(&contents).context("failed to parse ABI JSON")?;

    let abi_value = json
        .get("abi")
        .ok_or_else(|| anyhow!("'abi' field missing in artifact: {}", path.display()))?;

    serde_json::from_value(abi_value.clone()).context("failed to deserialize ABI")
}

/// Builds a lookup map from event signature (topic0) to event definition
fn build_event_map(
    factory_abi: &Abi,
    raffle_abi: &Abi,
    provider_abi: Option<&Abi>,
) -> anyhow::Result<HashMap<H256, EventDef>> {
    let mut map = HashMap::new();

    // Factory events
    register_event(
        &mut map,
        EventKind::RaffleCreated,
        factory_abi.event("RaffleCreated")?,
    );

    // Raffle events
    register_event(
        &mut map,
        EventKind::TicketsBought,
        raffle_abi.event("TicketsBought")?,
    );
    register_event(
        &mut map,
        EventKind::RaffleClosed,
        raffle_abi.event("RaffleClosed")?,
    );
    register_event(
        &mut map,
        EventKind::RandomnessRequested,
        raffle_abi.event("RandomnessRequested")?,
    );
    register_event(
        &mut map,
        EventKind::RandomnessFulfilled,
        raffle_abi.event("RandomnessFulfilled")?,
    );
    register_event(
        &mut map,
        EventKind::WinnerSelected,
        raffle_abi.event("WinnerSelected")?,
    );
    register_event(
        &mut map,
        EventKind::RefundClaimed,
        raffle_abi.event("RefundClaimed")?,
    );
    register_event(
        &mut map,
        EventKind::KeeperUpdated,
        raffle_abi.event("KeeperUpdated")?,
    );
    register_event(
        &mut map,
        EventKind::RefundsStarted,
        raffle_abi.event("RefundsStarted")?,
    );
    register_event(
        &mut map,
        EventKind::PayoutsCompleted,
        raffle_abi.event("PayoutsCompleted")?,
    );

    // DrandRandomnessProvider events (optional)
    if let Some(prov_abi) = provider_abi {
        if let Ok(event) = prov_abi.event("RandomnessRequested") {
            register_event(&mut map, EventKind::ProviderRandomnessRequested, event);
        }
        if let Ok(event) = prov_abi.event("RandomnessDelivered") {
            register_event(&mut map, EventKind::ProviderRandomnessDelivered, event);
        }
    }

    Ok(map)
}

fn register_event(map: &mut HashMap<H256, EventDef>, kind: EventKind, event: &Event) {
    map.insert(
        event.signature(),
        EventDef {
            kind,
            event: event.clone(),
        },
    );
}

// ============================================================================
// EVENT PROCESSING
// ============================================================================

/// Processes a single log entry and updates the database
///
/// Uses a database transaction to ensure atomicity.
/// Idempotent via ON CONFLICT DO NOTHING on unique constraints.
async fn process_log(
    db_pool: &PgPool,
    events_by_signature: &HashMap<H256, EventDef>,
    log_entry: &Log,
) -> anyhow::Result<()> {
    // Extract topic0 (event signature)
    let topic0 = log_entry.topics.first().cloned().unwrap_or_default();

    // Skip unknown events
    let Some(event_def) = events_by_signature.get(&topic0) else {
        return Ok(());
    };

    // Extract log metadata with proper error handling
    let tx_hash = log_entry
        .transaction_hash
        .ok_or_else(|| anyhow!("log missing transaction hash"))?;
    let log_index = log_entry
        .log_index
        .ok_or_else(|| anyhow!("log missing log index"))?;
    let block_number = log_entry
        .block_number
        .ok_or_else(|| anyhow!("log missing block number"))?;

    // Format for database storage (lowercase hex with 0x prefix)
    let tx_hash_hex = format!("{:#x}", tx_hash);
    let address_hex = format!("{:#x}", log_entry.address);
    let data_hex = format!("0x{}", hex::encode(log_entry.data.as_ref()));

    // Parse the log according to ABI
    let raw_log = RawLog {
        topics: log_entry.topics.clone(),
        data: log_entry.data.to_vec(),
    };
    let parsed = event_def
        .event
        .parse_log(raw_log)
        .context("failed to parse log")?;

    // Begin database transaction
    let mut db_tx = db_pool
        .begin()
        .await
        .context("failed to begin transaction")?;
    // Store raw logs for debugging and easy reprocessing.
    sqlx::query(
        "INSERT INTO events_raw (tx_hash, log_index, block_number, address, topic0, data)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (tx_hash, log_index) DO NOTHING",
    )
    .bind(&tx_hash_hex)
    .bind(log_index.as_u64() as i64)
    .bind(block_number.as_u64() as i64)
    .bind(&address_hex)
    .bind(format!("{:#x}", topic0))
    .bind(&data_hex)
    .execute(&mut *db_tx)
    .await?;

    match event_def.kind {
        EventKind::RaffleCreated => {
            let raffle_id = token_u256(&parsed, "raffleId")?;
            let raffle_address = token_address(&parsed, "raffle")?;
            let creator = token_address(&parsed, "creator")?;
            let end_time = token_u256(&parsed, "endTime")?;
            let ticket_price = token_u256(&parsed, "ticketPrice")?;
            let max_tickets = token_u256(&parsed, "maxTickets")?;
            let fee_bps = token_u256(&parsed, "feeBps")?;
            let fee_recipient = token_address(&parsed, "feeRecipient")?;

            let end_time = u256_to_datetime(end_time)?;
            sqlx::query(
                "INSERT INTO raffles
                (raffle_id, raffle_address, creator, end_time, ticket_price, max_tickets, fee_bps, fee_recipient, status)
                VALUES ($1, $2, $3, $4, $5::numeric, $6, $7, $8, $9)
                ON CONFLICT (raffle_id) DO UPDATE SET
                    raffle_address = excluded.raffle_address,
                    creator = excluded.creator,
                    end_time = excluded.end_time,
                    ticket_price = excluded.ticket_price,
                    max_tickets = excluded.max_tickets,
                    fee_bps = excluded.fee_bps,
                    fee_recipient = excluded.fee_recipient,
                    status = excluded.status,
                    updated_at = now()",
            )
            .bind(u256_to_i64(raffle_id)?)
            .bind(format!("{:#x}", raffle_address))
            .bind(format!("{:#x}", creator))
            .bind(end_time)
            .bind(ticket_price.to_string())
            .bind(u256_to_i64(max_tickets)?)
            .bind(u256_to_i64(fee_bps)?)
            .bind(format!("{:#x}", fee_recipient))
            .bind("ACTIVE")
            .execute(&mut *db_tx)
            .await?;
        }
        EventKind::TicketsBought => {
            let raffle_id = token_u256(&parsed, "raffleId")?;
            let buyer = token_address(&parsed, "buyer")?;
            let start_index = token_u256(&parsed, "startIndex")?;
            let end_index = token_u256(&parsed, "endIndex")?;
            let count = token_u256(&parsed, "count")?;
            let amount_paid = token_u256(&parsed, "amountPaid")?;

            let inserted = sqlx::query(
                "INSERT INTO purchases
                (raffle_id, buyer, start_index, end_index, count, amount, tx_hash, log_index, block_number)
                VALUES ($1, $2, $3, $4, $5, $6::numeric, $7, $8, $9)
                ON CONFLICT (tx_hash, log_index) DO NOTHING",
            )
            .bind(u256_to_i64(raffle_id)?)
            .bind(format!("{:#x}", buyer))
            .bind(u256_to_i64(start_index)?)
            .bind(u256_to_i64(end_index)?)
            .bind(u256_to_i64(count)?)
            .bind(amount_paid.to_string())
            .bind(&tx_hash_hex)
            .bind(log_index.as_u64() as i64)
            .bind(block_number.as_u64() as i64)
            .execute(&mut *db_tx)
            .await?
            .rows_affected();

            if inserted > 0 {
                sqlx::query(
                    "UPDATE raffles
                    SET total_tickets = total_tickets + $1,
                        pot = pot + $2::numeric,
                        updated_at = now()
                    WHERE raffle_id = $3",
                )
                .bind(u256_to_i64(count)?)
                .bind(amount_paid.to_string())
                .bind(u256_to_i64(raffle_id)?)
                .execute(&mut *db_tx)
                .await?;
            }
        }
        EventKind::RaffleClosed => {
            let raffle_id = token_u256(&parsed, "raffleId")?;
            let total_tickets = token_u256(&parsed, "totalTickets")?;
            let pot = token_u256(&parsed, "pot")?;
            sqlx::query(
                "UPDATE raffles
                SET status = $1,
                    total_tickets = $2,
                    pot = $3::numeric,
                    updated_at = now()
                WHERE raffle_id = $4",
            )
            .bind("CLOSED")
            .bind(u256_to_i64(total_tickets)?)
            .bind(pot.to_string())
            .bind(u256_to_i64(raffle_id)?)
            .execute(&mut *db_tx)
            .await?;
        }
        EventKind::RandomnessRequested => {
            let raffle_id = token_u256(&parsed, "raffleId")?;
            let request_id = token_u256(&parsed, "requestId")?;
            sqlx::query(
                "UPDATE raffles
                SET status = $1,
                    request_id = $2,
                    request_tx = $3,
                    updated_at = now()
                WHERE raffle_id = $4",
            )
            .bind("RANDOM_REQUESTED")
            .bind(request_id.to_string())
            .bind(&tx_hash_hex)
            .bind(u256_to_i64(raffle_id)?)
            .execute(&mut *db_tx)
            .await?;
        }
        EventKind::RandomnessFulfilled => {
            let raffle_id = token_u256(&parsed, "raffleId")?;
            let request_id = token_u256(&parsed, "requestId")?;
            let randomness = token_u256(&parsed, "randomness")?;
            sqlx::query(
                "UPDATE raffles
                SET status = $1,
                    request_id = $2,
                    randomness = $3,
                    randomness_tx = $4,
                    updated_at = now()
                WHERE raffle_id = $5",
            )
            .bind("RANDOM_FULFILLED")
            .bind(request_id.to_string())
            .bind(randomness.to_string())
            .bind(&tx_hash_hex)
            .bind(u256_to_i64(raffle_id)?)
            .execute(&mut *db_tx)
            .await?;
        }
        EventKind::WinnerSelected => {
            let raffle_id = token_u256(&parsed, "raffleId")?;
            let winner = token_address(&parsed, "winner")?;
            let winning_index = token_u256(&parsed, "winningIndex")?;
            sqlx::query(
                "UPDATE raffles
                SET status = $1,
                    winner = $2,
                    winning_index = $3,
                    finalized_tx = $4,
                    pot = 0,
                    updated_at = now()
                WHERE raffle_id = $5",
            )
            .bind("FINALIZED")
            .bind(format!("{:#x}", winner))
            .bind(u256_to_i64(winning_index)?)
            .bind(&tx_hash_hex)
            .bind(u256_to_i64(raffle_id)?)
            .execute(&mut *db_tx)
            .await?;
        }
        EventKind::RefundClaimed => {
            let raffle_id = token_u256(&parsed, "raffleId")?;
            let buyer = token_address(&parsed, "buyer")?;
            let amount = token_u256(&parsed, "amount")?;
            let inserted = sqlx::query(
                "INSERT INTO refunds
                (raffle_id, buyer, amount, tx_hash, log_index, block_number)
                VALUES ($1, $2, $3::numeric, $4, $5, $6)
                ON CONFLICT (tx_hash, log_index) DO NOTHING",
            )
            .bind(u256_to_i64(raffle_id)?)
            .bind(format!("{:#x}", buyer))
            .bind(amount.to_string())
            .bind(&tx_hash_hex)
            .bind(log_index.as_u64() as i64)
            .bind(block_number.as_u64() as i64)
            .execute(&mut *db_tx)
            .await?
            .rows_affected();

            if inserted > 0 {
                sqlx::query(
                    "UPDATE raffles
                    SET status = $1,
                        pot = pot - $2::numeric,
                        updated_at = now()
                    WHERE raffle_id = $3",
                )
                .bind("REFUNDING")
                .bind(amount.to_string())
                .bind(u256_to_i64(raffle_id)?)
                .execute(&mut *db_tx)
                .await?;
            }
        }
        EventKind::RefundsStarted => {
            let raffle_id = token_u256(&parsed, "raffleId")?;
            sqlx::query(
                "UPDATE raffles
                SET status = $1,
                    updated_at = now()
                WHERE raffle_id = $2",
            )
            .bind("REFUNDING")
            .bind(u256_to_i64(raffle_id)?)
            .execute(&mut *db_tx)
            .await
            .context("failed to update raffle to REFUNDING")?;
        }
        // Events we log but don't need to store derived state for
        EventKind::KeeperUpdated | EventKind::PayoutsCompleted => {}

        // DrandRandomnessProvider: RandomnessRequested(uint256 indexed requestId, uint256 indexed raffleId, address indexed raffle)
        EventKind::ProviderRandomnessRequested => {
            let request_id = token_u256(&parsed, "requestId")?;
            let raffle_id = token_u256(&parsed, "raffleId")?;
            let raffle_address = token_address(&parsed, "raffle")?;

            // Insert into randomness_requests table
            sqlx::query(
                "INSERT INTO randomness_requests
                (request_id, raffle_id, raffle_address, provider_address, tx_hash, log_index, block_number)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (tx_hash, log_index) DO NOTHING",
            )
            .bind(request_id.to_string())
            .bind(u256_to_i64(raffle_id).ok())  // May be too large
            .bind(format!("{:#x}", raffle_address))
            .bind(&address_hex)  // provider address is the log emitter
            .bind(&tx_hash_hex)
            .bind(log_index.as_u64() as i64)
            .bind(block_number.as_u64() as i64)
            .execute(&mut *db_tx)
            .await
            .context("failed to insert randomness request")?;

            // Update raffle with provider request info
            if let Ok(raffle_id_i64) = u256_to_i64(raffle_id) {
                sqlx::query(
                    "UPDATE raffles
                    SET provider_request_id = $1,
                        provider_request_tx = $2,
                        updated_at = now()
                    WHERE raffle_id = $3",
                )
                .bind(request_id.to_string())
                .bind(&tx_hash_hex)
                .bind(raffle_id_i64)
                .execute(&mut *db_tx)
                .await
                .context("failed to update raffle with provider request")?;
            }
        }

        // DrandRandomnessProvider: RandomnessDelivered(uint256 indexed requestId, uint256 randomness, bytes proof, address indexed raffle)
        EventKind::ProviderRandomnessDelivered => {
            let request_id = token_u256(&parsed, "requestId")?;
            let randomness = token_u256(&parsed, "randomness")?;
            let raffle_address = token_address(&parsed, "raffle")?;
            let proof = extract_bytes(&parsed, "proof").ok();

            let proof_hex = proof.map(|p| format!("0x{}", hex::encode(p)));

            // Insert into randomness_fulfillments table
            sqlx::query(
                "INSERT INTO randomness_fulfillments
                (request_id, randomness, proof, raffle_address, provider_address, tx_hash, log_index, block_number)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                ON CONFLICT (tx_hash, log_index) DO NOTHING",
            )
            .bind(request_id.to_string())
            .bind(randomness.to_string())
            .bind(&proof_hex)
            .bind(format!("{:#x}", raffle_address))
            .bind(&address_hex)  // provider address is the log emitter
            .bind(&tx_hash_hex)
            .bind(log_index.as_u64() as i64)
            .bind(block_number.as_u64() as i64)
            .execute(&mut *db_tx)
            .await
            .context("failed to insert randomness fulfillment")?;

            // Update raffle with provider fulfillment info (find by raffle_address)
            sqlx::query(
                "UPDATE raffles
                SET provider_fulfill_tx = $1,
                    proof_data = $2,
                    updated_at = now()
                WHERE raffle_address = $3",
            )
            .bind(&tx_hash_hex)
            .bind(&proof_hex)
            .bind(format!("{:#x}", raffle_address))
            .execute(&mut *db_tx)
            .await
            .context("failed to update raffle with provider fulfillment")?;
        }
    }

    db_tx
        .commit()
        .await
        .context("failed to commit transaction")?;
    Ok(())
}

// ============================================================================
// TOKEN EXTRACTION HELPERS
// ============================================================================

/// Extracts a U256 value from a parsed event log
fn extract_u256(parsed: &ethers::abi::Log, name: &str) -> anyhow::Result<U256> {
    let token = parsed
        .params
        .iter()
        .find(|param| param.name == name)
        .map(|param| &param.value)
        .ok_or_else(|| anyhow!("missing event parameter: {}", name))?;

    match token {
        Token::Uint(value) => Ok(*value),
        _ => Err(anyhow!("event parameter '{}' is not a uint", name)),
    }
}

/// Extracts an Address value from a parsed event log
fn extract_address(parsed: &ethers::abi::Log, name: &str) -> anyhow::Result<Address> {
    let token = parsed
        .params
        .iter()
        .find(|param| param.name == name)
        .map(|param| &param.value)
        .ok_or_else(|| anyhow!("missing event parameter: {}", name))?;

    match token {
        Token::Address(value) => Ok(*value),
        _ => Err(anyhow!("event parameter '{}' is not an address", name)),
    }
}

/// Extracts bytes data from a parsed event log
fn extract_bytes(parsed: &ethers::abi::Log, name: &str) -> anyhow::Result<Vec<u8>> {
    let token = parsed
        .params
        .iter()
        .find(|param| param.name == name)
        .map(|param| &param.value)
        .ok_or_else(|| anyhow!("missing event parameter: {}", name))?;

    match token {
        Token::Bytes(value) => Ok(value.clone()),
        _ => Err(anyhow!("event parameter '{}' is not bytes", name)),
    }
}

// Convenience aliases for more descriptive naming at call sites
#[inline]
fn token_u256(parsed: &ethers::abi::Log, name: &str) -> anyhow::Result<U256> {
    extract_u256(parsed, name)
}

#[inline]
fn token_address(parsed: &ethers::abi::Log, name: &str) -> anyhow::Result<Address> {
    extract_address(parsed, name)
}

// ============================================================================
// CONVERSION HELPERS
// ============================================================================

/// Converts U256 to i64, returning error if value overflows
fn u256_to_i64(value: U256) -> anyhow::Result<i64> {
    if value > U256::from(i64::MAX as u64) {
        return Err(anyhow!("U256 value {} overflows i64", value));
    }
    Ok(value.as_u64() as i64)
}

/// Converts a Unix timestamp U256 to DateTime<Utc>
fn u256_to_datetime(value: U256) -> anyhow::Result<DateTime<Utc>> {
    let seconds = u256_to_i64(value)?;
    DateTime::<Utc>::from_timestamp(seconds, 0)
        .ok_or_else(|| anyhow!("invalid Unix timestamp: {}", seconds))
}

// ============================================================================
// DATABASE HELPERS
// ============================================================================

/// Gets the last processed block from indexer_state
async fn get_last_processed_block(pool: &PgPool) -> anyhow::Result<u64> {
    let row = sqlx::query("SELECT last_processed_block FROM indexer_state WHERE id = 1")
        .fetch_one(pool)
        .await
        .context("failed to fetch indexer state")?;

    let value: i64 = row
        .try_get("last_processed_block")
        .context("failed to read last_processed_block")?;

    Ok(value as u64)
}

/// Updates the last processed block in indexer_state
async fn set_last_processed_block(pool: &PgPool, block: u64) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE indexer_state SET last_processed_block = $1, updated_at = now() WHERE id = 1",
    )
    .bind(block as i64)
    .execute(pool)
    .await
    .context("failed to update last processed block")?;
    Ok(())
}

/// Loads all known raffle addresses from the database
async fn load_raffle_addresses(pool: &PgPool) -> anyhow::Result<Vec<Address>> {
    let rows = sqlx::query("SELECT raffle_address FROM raffles ORDER BY raffle_id")
        .fetch_all(pool)
        .await
        .context("failed to fetch raffle addresses")?;

    let mut addresses = Vec::with_capacity(rows.len());
    for row in rows {
        let address_str: String = row
            .try_get("raffle_address")
            .context("failed to read raffle_address")?;
        let address = Address::from_str(&address_str)
            .with_context(|| format!("invalid address format: {}", address_str))?;
        addresses.push(address);
    }

    Ok(addresses)
}
