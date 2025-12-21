use crate::config::AppConfig;
use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use ethers::abi::{Abi, Event, RawLog, Token};
use ethers::providers::{Http, Provider};
use ethers::types::{Address, Filter, Log, H256, U256};
use sqlx::{PgPool, Row};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

const FACTORY_ARTIFACT: &str =
    "../contracts/artifacts/contracts/RaffleFactory.sol/RaffleFactory.json";
const RAFFLE_ARTIFACT: &str = "../contracts/artifacts/contracts/Raffle.sol/Raffle.json";

#[derive(Clone, Copy, Debug)]
enum EventKind {
    RaffleCreated,
    TicketsBought,
    RaffleClosed,
    RandomnessRequested,
    RandomnessFulfilled,
    WinnerSelected,
    RefundClaimed,
    KeeperUpdated,
    RefundsStarted,
    PayoutsCompleted,
}

#[derive(Clone, Debug)]
struct EventDef {
    kind: EventKind,
    event: Event,
}

pub async fn run(db_pool: PgPool, config: AppConfig) -> anyhow::Result<()> {
    let provider = Provider::<Http>::try_from(config.rpc_url.as_str())?
        .interval(Duration::from_millis(1500));
    let rpc_chain_id = ethers::providers::Middleware::get_chainid(&provider)
        .await?
        .as_u64();
    if rpc_chain_id != config.chain_id {
        return Err(anyhow!(
            "rpc chain id {} does not match config {}",
            rpc_chain_id,
            config.chain_id
        ));
    }
    let factory_abi = load_abi(FACTORY_ARTIFACT)?;
    let raffle_abi = load_abi(RAFFLE_ARTIFACT)?;
    let events_by_signature = build_event_map(&factory_abi, &raffle_abi)?;
    let factory_address = Address::from_str(&config.raffle_factory_address)?;

    tracing::info!(
        start_block = config.start_block,
        batch_size = config.indexer_batch_size,
        "indexer started"
    );

    loop {
        let latest = ethers::providers::Middleware::get_block_number(&provider)
            .await?
            .as_u64();
        let last_processed = get_last_processed_block(&db_pool).await?;
        let mut from_block = if last_processed == 0 {
            config.start_block
        } else {
            last_processed + 1
        };
        from_block = from_block.max(config.start_block);

        if from_block > latest {
            tokio::time::sleep(Duration::from_millis(config.indexer_poll_interval_ms)).await;
            continue;
        }

        let to_block = (from_block + config.indexer_batch_size - 1).min(latest);
        tracing::info!(from_block, to_block, "indexing batch");

        let factory_event_logs = fetch_logs(&provider, vec![factory_address], from_block, to_block)
            .await
            .context("fetch factory logs")?;
        for log_entry in factory_event_logs {
            process_log(&db_pool, &events_by_signature, &log_entry).await?;
        }

        let raffle_addresses = load_raffle_addresses(&db_pool).await?;
        if !raffle_addresses.is_empty() {
            let raffle_logs =
                fetch_logs(&provider, raffle_addresses, from_block, to_block).await?;
            for log_entry in raffle_logs {
                process_log(&db_pool, &events_by_signature, &log_entry).await?;
            }
        }

        set_last_processed_block(&db_pool, to_block).await?;
    }
}

async fn fetch_logs(
    provider: &Provider<Http>,
    addresses: Vec<Address>,
    from_block: u64,
    to_block: u64,
) -> anyhow::Result<Vec<Log>> {
    let filter = Filter::new()
        .address(addresses)
        .from_block(from_block)
        .to_block(to_block);
    let mut log_entries = ethers::providers::Middleware::get_logs(provider, &filter).await?;
    // Ensure deterministic processing order within the batch.
    log_entries.sort_by(|a, b| {
        let a_block = a.block_number.unwrap_or_default();
        let b_block = b.block_number.unwrap_or_default();
        match a_block.cmp(&b_block) {
            Ordering::Equal => a
                .log_index
                .unwrap_or_default()
                .cmp(&b.log_index.unwrap_or_default()),
            other => other,
        }
    });
    Ok(log_entries)
}

fn load_abi(relative_path: &str) -> anyhow::Result<Abi> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("read abi artifact {}", path.display()))?;
    let json: serde_json::Value = serde_json::from_str(&contents)?;
    let abi_value = json
        .get("abi")
        .ok_or_else(|| anyhow!("abi field missing in {}", path.display()))?;
    Ok(serde_json::from_value(abi_value.clone())?)
}

fn build_event_map(factory_abi: &Abi, raffle_abi: &Abi) -> anyhow::Result<HashMap<H256, EventDef>> {
    let mut map = HashMap::new();

    insert_event(
        &mut map,
        EventKind::RaffleCreated,
        factory_abi.event("RaffleCreated")?.clone(),
    );

    insert_event(
        &mut map,
        EventKind::TicketsBought,
        raffle_abi.event("TicketsBought")?.clone(),
    );
    insert_event(
        &mut map,
        EventKind::RaffleClosed,
        raffle_abi.event("RaffleClosed")?.clone(),
    );
    insert_event(
        &mut map,
        EventKind::RandomnessRequested,
        raffle_abi.event("RandomnessRequested")?.clone(),
    );
    insert_event(
        &mut map,
        EventKind::RandomnessFulfilled,
        raffle_abi.event("RandomnessFulfilled")?.clone(),
    );
    insert_event(
        &mut map,
        EventKind::WinnerSelected,
        raffle_abi.event("WinnerSelected")?.clone(),
    );
    insert_event(
        &mut map,
        EventKind::RefundClaimed,
        raffle_abi.event("RefundClaimed")?.clone(),
    );
    insert_event(
        &mut map,
        EventKind::KeeperUpdated,
        raffle_abi.event("KeeperUpdated")?.clone(),
    );
    insert_event(
        &mut map,
        EventKind::RefundsStarted,
        raffle_abi.event("RefundsStarted")?.clone(),
    );
    insert_event(
        &mut map,
        EventKind::PayoutsCompleted,
        raffle_abi.event("PayoutsCompleted")?.clone(),
    );

    Ok(map)
}

fn insert_event(map: &mut HashMap<H256, EventDef>, kind: EventKind, event: Event) {
    map.insert(event.signature(), EventDef { kind, event });
}

async fn process_log(
    db_pool: &PgPool,
    events_by_signature: &HashMap<H256, EventDef>,
    log_entry: &Log,
) -> anyhow::Result<()> {
    let topic0 = log_entry.topics.get(0).cloned().unwrap_or_default();
    let Some(event_def) = events_by_signature.get(&topic0) else {
        return Ok(());
    };

    let tx_hash = log_entry
        .transaction_hash
        .context("log missing tx hash")?;
    let log_index = log_entry.log_index.context("log missing log index")?;
    let block_number = log_entry
        .block_number
        .context("log missing block number")?;
    let tx_hash_hex = format!("{:#x}", tx_hash);
    let address_hex = format!("{:#x}", log_entry.address);
    let data_hex = format!("0x{}", hex::encode(log_entry.data.as_ref()));

    let raw_log = RawLog {
        topics: log_entry.topics.clone(),
        data: log_entry.data.to_vec(),
    };
    let parsed = event_def.event.parse_log(raw_log)?;

    let mut db_tx = db_pool.begin().await?;
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
        EventKind::KeeperUpdated => {}
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
            .await?;
        }
        EventKind::PayoutsCompleted => {}
    }

    db_tx.commit().await?;
    Ok(())
}

fn token_u256(parsed: &ethers::abi::Log, name: &str) -> anyhow::Result<U256> {
    let token = parsed
        .params
        .iter()
        .find(|param| param.name == name)
        .map(|param| &param.value)
        .ok_or_else(|| anyhow!("missing param {}", name))?;
    match token {
        Token::Uint(value) => Ok(*value),
        _ => Err(anyhow!("param {} not uint", name)),
    }
}

fn token_address(parsed: &ethers::abi::Log, name: &str) -> anyhow::Result<Address> {
    let token = parsed
        .params
        .iter()
        .find(|param| param.name == name)
        .map(|param| &param.value)
        .ok_or_else(|| anyhow!("missing param {}", name))?;
    match token {
        Token::Address(value) => Ok(*value),
        _ => Err(anyhow!("param {} not address", name)),
    }
}

fn u256_to_i64(value: U256) -> anyhow::Result<i64> {
    if value > U256::from(i64::MAX) {
        return Err(anyhow!("value overflows i64"));
    }
    Ok(value.as_u64() as i64)
}

fn u256_to_datetime(value: U256) -> anyhow::Result<DateTime<Utc>> {
    let seconds = u256_to_i64(value)?;
    DateTime::<Utc>::from_timestamp(seconds, 0).ok_or_else(|| anyhow!("invalid timestamp"))
}

async fn get_last_processed_block(pool: &PgPool) -> anyhow::Result<u64> {
    let row = sqlx::query("SELECT last_processed_block FROM indexer_state WHERE id = 1")
        .fetch_one(pool)
        .await?;
    let value: i64 = row.try_get("last_processed_block")?;
    Ok(value as u64)
}

async fn set_last_processed_block(pool: &PgPool, block: u64) -> anyhow::Result<()> {
    sqlx::query("UPDATE indexer_state SET last_processed_block = $1, updated_at = now() WHERE id = 1")
        .bind(block as i64)
        .execute(pool)
        .await?;
    Ok(())
}

async fn load_raffle_addresses(pool: &PgPool) -> anyhow::Result<Vec<Address>> {
    let rows = sqlx::query("SELECT raffle_address FROM raffles ORDER BY raffle_id")
        .fetch_all(pool)
        .await?;
    let mut addresses = Vec::with_capacity(rows.len());
    for row in rows {
        let raffle_address: String = row.try_get("raffle_address")?;
        let address = Address::from_str(&raffle_address)?;
        addresses.push(address);
    }
    Ok(addresses)
}
