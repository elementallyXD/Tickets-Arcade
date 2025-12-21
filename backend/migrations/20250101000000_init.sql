CREATE TABLE IF NOT EXISTS indexer_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    last_processed_block BIGINT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO indexer_state (id, last_processed_block)
VALUES (1, 0)
ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS raffles (
    raffle_id BIGINT PRIMARY KEY,
    raffle_address TEXT NOT NULL UNIQUE,
    creator TEXT NOT NULL,
    end_time TIMESTAMPTZ,
    ticket_price NUMERIC NOT NULL,
    max_tickets INTEGER NOT NULL,
    fee_bps INTEGER NOT NULL,
    fee_recipient TEXT NOT NULL,
    status TEXT NOT NULL,
    total_tickets INTEGER NOT NULL DEFAULT 0,
    pot NUMERIC NOT NULL DEFAULT 0,
    request_id TEXT,
    request_tx TEXT,
    randomness TEXT,
    randomness_tx TEXT,
    winning_index INTEGER,
    winner TEXT,
    finalized_tx TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_raffles_status ON raffles (status);

CREATE TABLE IF NOT EXISTS purchases (
    id BIGSERIAL PRIMARY KEY,
    raffle_id BIGINT NOT NULL REFERENCES raffles (raffle_id),
    buyer TEXT NOT NULL,
    start_index INTEGER NOT NULL,
    end_index INTEGER NOT NULL,
    count INTEGER NOT NULL,
    amount NUMERIC NOT NULL,
    tx_hash TEXT NOT NULL,
    log_index INTEGER NOT NULL,
    block_number BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tx_hash, log_index)
);

CREATE INDEX IF NOT EXISTS idx_purchases_raffle_id ON purchases (raffle_id);
CREATE INDEX IF NOT EXISTS idx_purchases_buyer ON purchases (buyer);

CREATE TABLE IF NOT EXISTS refunds (
    id BIGSERIAL PRIMARY KEY,
    raffle_id BIGINT NOT NULL REFERENCES raffles (raffle_id),
    buyer TEXT NOT NULL,
    amount NUMERIC NOT NULL,
    tx_hash TEXT NOT NULL,
    log_index INTEGER NOT NULL,
    block_number BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tx_hash, log_index)
);

CREATE INDEX IF NOT EXISTS idx_refunds_raffle_id ON refunds (raffle_id);
CREATE INDEX IF NOT EXISTS idx_refunds_buyer ON refunds (buyer);

CREATE TABLE IF NOT EXISTS events_raw (
    tx_hash TEXT NOT NULL,
    log_index INTEGER NOT NULL,
    block_number BIGINT NOT NULL,
    address TEXT NOT NULL,
    topic0 TEXT NOT NULL,
    data TEXT NOT NULL,
    inserted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tx_hash, log_index)
);
