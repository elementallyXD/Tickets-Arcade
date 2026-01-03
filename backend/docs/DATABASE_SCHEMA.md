# Database Schema

Migrations live in `backend/migrations` and are applied with SQLx:
```bash
sqlx migrate run --source backend/migrations
```

## Tables

### indexer_state
Tracks indexer progress.

Columns:
- `id` (integer, fixed value `1`)
- `last_processed_block` (bigint)
- `updated_at` (timestamptz)

### raffles
Derived raffle metadata and status.

Columns:
- `raffle_id` (bigint, primary key)
- `raffle_address` (text, unique)
- `creator` (text)
- `end_time` (timestamptz)
- `ticket_price` (numeric)
- `max_tickets` (int)
- `fee_bps` (int)
- `fee_recipient` (text)
- `status` (text)
- `total_tickets` (int)
- `pot` (numeric)
- `request_id` (text)
- `request_tx` (text)
- `randomness` (text)
- `randomness_tx` (text)
- `winning_index` (int)
- `winner` (text)
- `finalized_tx` (text)
- `created_at` (timestamptz)
- `updated_at` (timestamptz)

Indexes:
- `idx_raffles_status` on `status`

### purchases
Ticket purchase ranges for each raffle.

Columns:
- `id` (bigserial, primary key)
- `raffle_id` (bigint, FK to `raffles.raffle_id`)
- `buyer` (text)
- `start_index` (int)
- `end_index` (int)
- `count` (int)
- `amount` (numeric)
- `tx_hash` (text)
- `log_index` (int)
- `block_number` (bigint)
- `created_at` (timestamptz)

Unique constraints:
- `UNIQUE (tx_hash, log_index)`

Indexes:
- `idx_purchases_raffle_id`
- `idx_purchases_buyer`

### refunds
Refund claims per raffle.

Columns:
- `id` (bigserial, primary key)
- `raffle_id` (bigint, FK to `raffles.raffle_id`)
- `buyer` (text)
- `amount` (numeric)
- `tx_hash` (text)
- `log_index` (int)
- `block_number` (bigint)
- `created_at` (timestamptz)

Unique constraints:
- `UNIQUE (tx_hash, log_index)`

Indexes:
- `idx_refunds_raffle_id`
- `idx_refunds_buyer`

### events_raw
Raw log storage for debugging and reprocessing.

Columns:
- `tx_hash` (text)
- `log_index` (int)
- `block_number` (bigint)
- `address` (text)
- `topic0` (text)
- `data` (text)
- `inserted_at` (timestamptz)

Unique constraints:
- `UNIQUE (tx_hash, log_index)`

### randomness_requests

Stores `RandomnessRequested` events from the DrandRandomnessProvider contract.

Columns:
- `id` (bigserial, primary key)
- `request_id` (text)
- `raffle_id` (bigint, optional)
- `raffle_address` (text)
- `provider_address` (text)
- `tx_hash` (text)
- `log_index` (int)
- `block_number` (bigint)
- `created_at` (timestamptz)

Indexes:
- `idx_randomness_requests_request_id`
- `idx_randomness_requests_raffle_id`
- `idx_randomness_requests_raffle_address`

### randomness_fulfillments

Stores `RandomnessDelivered` events from the DrandRandomnessProvider contract, including proof data.

Columns:
- `id` (bigserial, primary key)
- `request_id` (text)
- `randomness` (text)
- `proof` (text)
- `raffle_address` (text)
- `provider_address` (text)
- `tx_hash` (text)
- `log_index` (int)
- `block_number` (bigint)
- `created_at` (timestamptz)

Indexes:
- `idx_randomness_fulfillments_request_id`
- `idx_randomness_fulfillments_raffle_address`
