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

- **Rust** (2024 edition, 1.84+)
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
| `RAFFLE_FACTORY_ADDRESS` | ✅ | - | RaffleFactory contract address |
| `RPC_URL` | ❌ | `https://rpc.testnet.arc.network` | Arc L1 RPC endpoint |
| `CHAIN_ID` | ❌ | `5042002` | Chain ID (Arc testnet) |
| `START_BLOCK` | ❌ | `0` | Block to start indexing from |
| `RANDOMNESS_PROVIDER_ADDRESS` | ❌ | - | DrandRandomnessProvider contract address |
| `EXPLORER_BASE_URL` | ❌ | `https://testnet.arcscan.app` | Block explorer base URL for tx links |
| `BIND_ADDR` | ❌ | `0.0.0.0:8080` | Address to bind the HTTP server |
| `INDEXER_BATCH_SIZE` | ❌ | `2000` | Max blocks per RPC query |
| `INDEXER_POLL_INTERVAL_MS` | ❌ | `3000` | Polling interval in milliseconds |

### Randomness Provider Configuration

When `RANDOMNESS_PROVIDER_ADDRESS` is set, the indexer will:
1. Load the DrandRandomnessProvider ABI from `contracts/artifacts/contracts/DrandRandomnessProvider.sol/DrandRandomnessProvider.json`
2. Index `RandomnessRequested` and `RandomnessDelivered` events
3. Store requests/fulfillments in separate tables
4. Link provider data back to raffles via `provider_request_id`, `provider_request_tx`, `provider_fulfill_tx`, and `proof_data` columns

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
- `status` (optional): `ACTIVE`, `CLOSED`, `RANDOM_REQUESTED`, `RANDOM_FULFILLED`, `FINALIZED`, `REFUNDING`

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
Returns the cryptographic verification proof for drawn raffles. Includes:
- Randomness value
- Winning index and winner address
- Transaction links for request, randomness, and finalization
- **Provider data**: `provider_request_id`, `provider_request_tx`, `provider_fulfill_tx`, `proof_data` (when DrandRandomnessProvider is configured)

### List Randomness Requests
```
GET /v1/randomness/requests?limit=50&offset=0&raffle_address=0x...&raffle_id=1
```
Query parameters:
- `limit` (optional, max 100, default 50)
- `offset` (optional, default 0)
- `raffle_address` (optional): Filter by raffle contract address
- `raffle_id` (optional): Filter by raffle ID

Returns all `RandomnessRequested` events from the DrandRandomnessProvider.

### Get Randomness Request by ID
```
GET /v1/randomness/requests/{request_id}
```
Returns a specific randomness request by its provider request ID.

### List Randomness Fulfillments
```
GET /v1/randomness/fulfillments?limit=50&offset=0&raffle_address=0x...
```
Query parameters:
- `limit` (optional, max 100, default 50)
- `offset` (optional, default 0)
- `raffle_address` (optional): Filter by raffle contract address

Returns all `RandomnessDelivered` events from the DrandRandomnessProvider, including proof data.

## Development

See [TESTING.md](./TESTING.md) for detailed testing and debugging instructions.

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
- `raffles` - Raffle metadata, status, and provider linkage (`provider_request_id`, `provider_request_tx`, `provider_fulfill_tx`, `proof_data`)
- `purchases` - Individual ticket purchases with ranges
- `randomness_requests` - DrandRandomnessProvider `RandomnessRequested` events
- `randomness_fulfillments` - DrandRandomnessProvider `RandomnessDelivered` events with proof data
- `indexer_state` - Last processed block for resumable indexing

## Security

Security protections in the backend:

- **SQL Injection Prevention**: All queries use parameterized statements via sqlx
- **Pagination Limits**: Enforced MAX_PAGE_LIMIT = 100 to prevent DoS
- **RPC Timeouts**: 30-second timeout on blockchain calls
- **Error Handling**: Database errors logged but not exposed to clients
- **Graceful Shutdown**: Clean termination on SIGTERM/Ctrl+C
- **Input Validation**: All addresses validated for proper Ethereum format

For contract security, see [contracts/docs/SECURITY_MODEL.md](../contracts/docs/SECURITY_MODEL.md).

## License

MIT
