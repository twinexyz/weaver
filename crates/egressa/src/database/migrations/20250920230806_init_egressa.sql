-- Initialize egressa database schema

-- Add migration script here
-- This migration initializes the database schema for the egressa service

-- Example: Create tables for egressa functionality
-- CREATE TABLE IF NOT EXISTS egressa_config (
--     id SERIAL PRIMARY KEY,
--     key VARCHAR(255) NOT NULL UNIQUE,
--     value TEXT NOT NULL,
--     created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
--     updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
-- );

-- Example: Create indexes for performance
-- CREATE INDEX IF NOT EXISTS idx_egressa_config_key ON egressa_config(key);

-- Add your egressa-specific database schema here

CREATE TABLE IF NOT EXISTS withdrawal_events (
    id SERIAL PRIMARY KEY,
    event_type VARCHAR(255) NOT NULL,
    l1_chain_id BIGINT NOT NULL,
    l2_transaction_hash VARCHAR(255) NOT NULL,
    l1_token VARCHAR(255) NOT NULL,
    l1_address VARCHAR(255) NOT NULL,
    public_values BYTEA NOT NULL,
    l1_txn_hash VARCHAR(255) NOT NULL,
    is_processed BOOLEAN NOT NULL DEFAULT FALSE,
    proof BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);