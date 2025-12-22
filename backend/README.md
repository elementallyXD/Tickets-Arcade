# Ticket Arcade Backend

Rust + Axum backend that indexes Arc L1 blockchain events and serves REST APIs for the React frontend.

## Overview

This backend:
1. **Indexes on-chain events** from the Arc L1 testnet (RaffleCreated, TicketsPurchased, WinnerDrawn, etc.)
2. **Stores data in PostgreSQL** for efficient querying
3. **Exposes REST APIs** for the frontend to fetch raffle data

### Architecture

```
┌─────────────────┐     ┌──────────────┐     ┌────────────────┐
│  Arc L1 RPC     │────▶│   Indexer    │────▶│   PostgreSQL   │
│  (Blockchain)   │     │              │     │                │
└─────────────────┘     └──────────────┘     └───────┬────────┘
                                                      │
                        ┌──────────────┐              │
                        │   Axum API   │◀─────────────┘
                        │   Server     │
                        └──────┬───────┘
                               │
                        ┌──────▼───────┐
                        │   Frontend   │
                        │   (React)    │
                        └──────────────┘
```

## Prerequisites

- **Rust** (2024 edition, 1.75+)
- **Docker Desktop** (for local Postgres)
- **sqlx-cli** (for migrations)

Install sqlx-cli:
```bash
cargo install sqlx-cli --no-default-features --features postgres
```

## Quick Start

### 1. Configure Environment

```bash
cd backend
cp .env.example .env
```

Edit `.env` with your values (see [Environment Variables](#environment-variables) below).

### 2. Start PostgreSQL

```bash
docker compose up -d
```

### 3. Run Migrations

```bash
sqlx migrate run --source migrations
```

### 4. Run the Backend

```bash
cargo run
```

The server starts on `BIND_ADDR` (default `0.0.0.0:8080`) and automatically begins indexing.

## Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `DATABASE_URL` | ✅ | - | PostgreSQL connection string |
| `RPC_URL` | ✅ | - | Arc L1 RPC endpoint |
| `CHAIN_ID` | ✅ | - | Chain ID (Arc testnet: `5042002`) |
| `START_BLOCK` | ✅ | - | Block to start indexing from |
| `RAFFLE_FACTORY_ADDRESS` | ✅ | - | RaffleFactory contract address |
| `RANDOMNESS_PROVIDER_ADDRESS` | ✅ | - | RandomnessProvider contract address |
| `EXPLORER_BASE_URL` | ❌ | - | Block explorer base URL for tx links |
| `BIND_ADDR` | ❌ | `0.0.0.0:8080` | Address to bind the HTTP server |
| `INDEXER_BATCH_SIZE` | ❌ | `500` | Max blocks per RPC query |
| `INDEXER_POLL_INTERVAL_MS` | ❌ | `3000` | Polling interval in milliseconds |

### Security Notes

- `DATABASE_URL` is automatically redacted in debug logs
- All environment variables are validated at startup
- Address fields are validated for proper Ethereum address format

## API Reference

### Health Check
```
GET /health
Response: { "status": "ok" }
```

### List Raffles
```
GET /v1/raffles?limit=50&offset=0&status=ACTIVE
```
Query parameters:
- `limit` (optional, max 100, default 50)
- `offset` (optional, default 0)
- `status` (optional): `OPEN`, `ACTIVE`, `DRAWING`, `DRAWN`, `COMPLETED`, `REFUNDING`, `REFUNDED`

### Get Raffle Details
```
GET /v1/raffles/{raffle_id}
```

### List Ticket Purchases
```
GET /v1/raffles/{raffle_id}/purchases?limit=50&offset=0
```
Returns all ticket purchase events with ranges.

### Get Raffle Proof
```
GET /v1/raffles/{raffle_id}/proof
```
Returns the cryptographic verification proof for drawn raffles.

## Development

### Running Tests
```bash
cargo test
```

### Linting
```bash
cargo clippy
```

### Formatting
```bash
cargo fmt
```

### Reset Indexer

To re-index from the start block, update the database:
```sql
UPDATE indexer_state SET last_processed_block = 0;
```

## Troubleshooting

### "Connection refused" when starting

Ensure PostgreSQL is running:
```bash
docker compose ps
docker compose up -d
```

### "Database connection timed out"

Check that `DATABASE_URL` is correct and the database container is healthy:
```bash
docker compose logs postgres
```

### Indexer not finding events

1. Verify `START_BLOCK` is before your first transaction
2. Check `RAFFLE_FACTORY_ADDRESS` matches your deployed contract
3. Review logs for RPC errors

### High memory usage

Reduce `INDEXER_BATCH_SIZE` to process fewer blocks per query.

## Database Schema

Key tables:
- `raffles` - Raffle metadata and status
- `raffle_purchases` - Individual ticket purchases with ranges
- `indexer_state` - Last processed block for resumable indexing

## Security

See [SECURITY_REVIEW_BACKEND.md](./SECURITY_REVIEW_BACKEND.md) for detailed security analysis and mitigations.

Key protections:
- All SQL queries use parameterized statements (no SQL injection)
- Pagination enforced with MAX_PAGE_LIMIT = 100
- RPC calls have 30-second timeouts
- Database errors are logged but not exposed to clients
- Graceful shutdown on SIGTERM/Ctrl+C

## License

MIT
