# Backend Architecture

## Overview

The backend is a single Rust binary that runs two concurrent components:

| Component | Purpose |
|-----------|---------|
| **Indexer** | Scans Arc L1 blockchain logs and stores events in PostgreSQL |
| **HTTP API** | Serves raffle data to the frontend via REST endpoints |

The database contains a **derived view** of on-chain events. The blockchain is the source of truth.

```
┌─────────────────┐     ┌──────────────┐     ┌────────────────┐
│  Arc L1 RPC     │────▶│   Indexer    │────▶│   PostgreSQL   │
│  (Blockchain)   │     │              │     │                │
└─────────────────┘     └──────────────┘     └───────┬────────┘
                                                      │
                        ┌──────────────┐              │
                        │   Axum API   │◀─────────────┘
                        │   Server     │
                        └──────────────┘
```

---

## Indexer Flow

1. **Load ABIs** from `contracts/artifacts/` on startup
2. **Verify chain ID** against RPC (prevents wrong-network indexing)
3. **Fetch latest block** from the RPC
4. **Read checkpoint** from `indexer_state.last_processed_block`
5. **Query logs in batches:**
   - Factory logs (discover new raffles via `RaffleCreated`)
   - Provider logs (track randomness requests/fulfillments)
   - Raffle logs (all known raffle addresses)
6. **Process each log:**
   - Decode event by signature (topic0)
   - Store raw copy in `events_raw`
   - Update derived tables (`raffles`, `purchases`, `refunds`, `randomness_*`)
7. **Update checkpoint** in `indexer_state` after each batch

### Deterministic Ordering

Logs are sorted by `(block_number, log_index)` before processing to ensure consistent state regardless of RPC response order.

---

## Event Decoding

Event decoding uses ethers-rs with ABI definitions from Hardhat artifacts:

| Contract | Events |
|----------|--------|
| RaffleFactory | `RaffleCreated` |
| Raffle | `TicketsBought`, `RaffleClosed`, `RandomnessRequested`, `RandomnessFulfilled`, `WinnerSelected`, `RefundClaimed`, `RefundsStarted`, `PayoutsCompleted` |
| DrandRandomnessProvider | `RandomnessRequested`, `RandomnessDelivered` |

---

## Data Model

The indexer maintains derived state in PostgreSQL:

| Table | Purpose |
|-------|---------|
| `raffles` | Raffle metadata, status, totals, winner info |
| `purchases` | Ticket purchase ranges per buyer |
| `refunds` | Refund claims |
| `randomness_requests` | Provider-level randomness requests |
| `randomness_fulfillments` | Provider-level randomness deliveries with proofs |
| `events_raw` | Raw event logs for debugging |
| `indexer_state` | Last processed block checkpoint |

---

## API Layer

The API is built with [Axum](https://github.com/tokio-rs/axum) and reads directly from PostgreSQL:

| Endpoint | Purpose |
|----------|---------|
| `/v1/raffles` | List raffles with filtering and pagination |
| `/v1/raffles/:id` | Get raffle details |
| `/v1/raffles/:id/purchases` | Get ticket purchase ranges |
| `/v1/raffles/:id/proof` | Get verification proof data |
| `/v1/randomness/requests` | List provider randomness requests |
| `/v1/randomness/fulfillments` | List provider randomness fulfillments |

### Security Features

- **Parameterized queries:** All SQL uses bind parameters (no injection risk)
- **Pagination limits:** Maximum 100 items per request
- **Error sanitization:** Database errors are logged but not exposed to clients
- **Request timeouts:** 30-second timeout on RPC calls

---

## Reorg and Restart Behavior

**Current behavior:** Minimal reorg handling

- Indexer stores only the last processed block number
- Unique constraints (`tx_hash`, `log_index`) prevent duplicate inserts on restart
- Idempotent upserts allow safe reprocessing

**Known limitation:** Deep reorgs (> few blocks) may cause stale data. Future enhancement: store block hashes and reprocess a confirmation window.

---

## Configuration

Key environment variables:

| Variable | Purpose |
|----------|---------|
| `INDEXER_BATCH_SIZE` | Blocks per RPC query (default: 2000) |
| `INDEXER_POLL_INTERVAL_MS` | Poll frequency (default: 3000ms) |
| `RPC_TIMEOUT` | Per-call timeout (hardcoded: 30s) |

---

## Operational Notes

1. **Single process:** Indexer and API run in the same binary
2. **Graceful shutdown:** Handles SIGTERM/Ctrl+C cleanly
3. **ABI dependency:** Requires compiled artifacts in `contracts/artifacts/`
4. **Database migrations:** Must run before starting (`sqlx migrate run`)
5. **Logging:** Uses `tracing` with configurable log levels via `RUST_LOG`
