-- Add migration script here
CREATE TYPE l1_action AS ENUM (
    'commit_batch',
    'finalize_batch',
    'finalize_transactions'
);

CREATE TABLE l1_transaction_tracker (
    tx_hash BYTEA PRIMARY KEY,
    raw_tx BYTEA NOT NULL,
    nonce BIGINT NOT NULL,
    chain_id BIGINT NOT NULL,
    action l1_action NOT NULL,
    batch_number BIGINT NOT NULL,  -- Added for batch association
    status TEXT NOT NULL CHECK (status IN ('submitted', 'confirmed', 'failed')),
    submitted_at TIMESTAMP NOT NULL DEFAULT NOW(),
    confirmed_at TIMESTAMP,
    failed_at TIMESTAMP,
    block_number BIGINT,
    gas_used  BIGINT,
    effective_gas_price BIGINT,
    metadata JSONB
);

-- Improved indexes
CREATE INDEX idx_l1_tx_tracker_chain_action_status ON l1_transaction_tracker(chain_id, action, status);
CREATE INDEX idx_l1_tx_tracker_batch_number ON l1_transaction_tracker(batch_number) WHERE batch_number IS NOT NULL;
CREATE INDEX idx_l1_tx_tracker_chain_nonce ON l1_transaction_tracker(chain_id, nonce);
