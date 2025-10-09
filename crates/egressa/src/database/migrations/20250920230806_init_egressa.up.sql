-- Create enum type for withdrawal event types
CREATE TYPE withdrawal_event_type AS ENUM ('ForcedWithdraw', 'Withdraw', 'Deposit');

CREATE TABLE IF NOT EXISTS withdrawal_events (
    id SERIAL PRIMARY KEY,
    event_type withdrawal_event_type NOT NULL,
    l1_chain_id BIGINT NOT NULL,
    l2_transaction_hash VARCHAR(255) NOT NULL,
    l1_token VARCHAR(255) NOT NULL,
    l1_address VARCHAR(255) NOT NULL,
    public_values BYTEA NOT NULL,
    is_processed BOOLEAN NOT NULL DEFAULT FALSE,
    is_failed BOOLEAN NOT NULL DEFAULT FALSE,
    failure_reason TEXT,
    process_txn_hash VARCHAR(255),
    proof BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_withdrawal_events_l2_txn_hash ON withdrawal_events(l2_transaction_hash);
CREATE INDEX IF NOT EXISTS idx_withdrawal_events_is_processed ON withdrawal_events(is_processed);
CREATE INDEX IF NOT EXISTS idx_withdrawal_events_is_failed ON withdrawal_events(is_failed);
CREATE INDEX IF NOT EXISTS idx_withdrawal_events_l1_chain_id ON withdrawal_events(l1_chain_id);
CREATE INDEX IF NOT EXISTS idx_withdrawal_events_event_type ON withdrawal_events(event_type);
