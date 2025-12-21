# Ticket Arcade Backend

Rust + Axum backend that indexes Arc L1 events and serves REST APIs for the frontend.

## Prerequisites
- Rust (edition 2021 toolchain)
- Docker Desktop (for local Postgres)
- Optional: sqlx-cli for running migrations

## Configuration
Create a `.env` file in `backend/` using the example:
```
copy .env.example .env
```

Key env vars:
- `RPC_URL` (Arc testnet RPC)
- `CHAIN_ID` (Arc testnet = `5042002`)
- `START_BLOCK` (indexer start block; current deploy uses `17542046`)
- `DATABASE_URL` (Postgres connection string)
- `RAFFLE_FACTORY_ADDRESS`
- `RANDOMNESS_PROVIDER_ADDRESS`
- `EXPLORER_BASE_URL`
- `BIND_ADDR`
- `INDEXER_BATCH_SIZE`
- `INDEXER_POLL_INTERVAL_MS`

## Local Postgres (Docker)
From the repo root:
```
docker compose -f backend/docker-compose.yml up -d
```

## Migrations (SQLx)
Install sqlx-cli once:
```
cargo install sqlx-cli --no-default-features --features postgres
```

Run migrations:
```
sqlx migrate run --source backend/migrations
```

## Run the backend
From `backend/`:
```
cargo run
```

The server listens on `BIND_ADDR` and starts the indexer automatically.

## API (v1)
Health:
```
GET /health
```

Raffles list:
```
GET /v1/raffles?limit=50&offset=0&status=ACTIVE
```

Raffle details:
```
GET /v1/raffles/{raffle_id}
```

Ticket ranges:
```
GET /v1/raffles/{raffle_id}/purchases?limit=50&offset=0
```

Proof payload:
```
GET /v1/raffles/{raffle_id}/proof
```

## Notes
- ABI files are loaded from `contracts/artifacts` at runtime.
- Indexer is idempotent via unique constraints on `(tx_hash, log_index)`.
- Reset the indexer by setting `indexer_state.last_processed_block` to `0`.
