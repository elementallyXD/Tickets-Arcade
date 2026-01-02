-- Migration: Add DrandRandomnessProvider event tracking tables
-- 
-- These tables store events from the DrandRandomnessProvider contract,
-- allowing full verification of the randomness flow from request to delivery.

-- Tracks RandomnessRequested events from DrandRandomnessProvider
-- Maps request_id -> raffle for linking randomness to specific raffles
CREATE TABLE IF NOT EXISTS randomness_requests (
    id BIGSERIAL PRIMARY KEY,
    -- The request ID assigned by the provider
    request_id TEXT NOT NULL,
    -- The raffle ID that requested randomness (from event)
    raffle_id BIGINT,
    -- The raffle contract address (consumer)
    raffle_address TEXT NOT NULL,
    -- The randomness provider contract address
    provider_address TEXT NOT NULL,
    -- Transaction info for verification
    tx_hash TEXT NOT NULL,
    log_index INTEGER NOT NULL,
    block_number BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Prevent duplicate event processing
    UNIQUE (tx_hash, log_index)
);

CREATE INDEX IF NOT EXISTS idx_randomness_requests_request_id 
    ON randomness_requests (request_id);
CREATE INDEX IF NOT EXISTS idx_randomness_requests_raffle_id 
    ON randomness_requests (raffle_id);
CREATE INDEX IF NOT EXISTS idx_randomness_requests_raffle_address 
    ON randomness_requests (raffle_address);

-- Tracks RandomnessDelivered events from DrandRandomnessProvider
-- Contains the actual randomness value and proof for verification
CREATE TABLE IF NOT EXISTS randomness_fulfillments (
    id BIGSERIAL PRIMARY KEY,
    -- The request ID this fulfillment is for
    request_id TEXT NOT NULL,
    -- The randomness value (stored as decimal string for precision)
    randomness TEXT NOT NULL,
    -- Optional proof data (hex-encoded bytes)
    proof TEXT,
    -- The raffle contract address that received the randomness
    raffle_address TEXT NOT NULL,
    -- The randomness provider contract address
    provider_address TEXT NOT NULL,
    -- Transaction info for verification
    tx_hash TEXT NOT NULL,
    log_index INTEGER NOT NULL,
    block_number BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Prevent duplicate event processing
    UNIQUE (tx_hash, log_index)
);

CREATE INDEX IF NOT EXISTS idx_randomness_fulfillments_request_id 
    ON randomness_fulfillments (request_id);
CREATE INDEX IF NOT EXISTS idx_randomness_fulfillments_raffle_address 
    ON randomness_fulfillments (raffle_address);

-- Add provider-related columns to raffles table for direct linking
ALTER TABLE raffles 
    ADD COLUMN IF NOT EXISTS provider_request_id TEXT,
    ADD COLUMN IF NOT EXISTS provider_request_tx TEXT,
    ADD COLUMN IF NOT EXISTS provider_fulfill_tx TEXT,
    ADD COLUMN IF NOT EXISTS proof_data TEXT;
