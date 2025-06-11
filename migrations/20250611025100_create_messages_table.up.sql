CREATE TABLE IF NOT EXISTS messages (
    nonce BIGINT NOT NULL,
    message_type VARCHAR(20) NOT NULL,
    chain_id BIGINT NOT NULL,
    slot_or_block_number BIGINT NOT NULL,
    bank_hash_or_receipt_root BYTEA NOT NULL,
    public_values BYTEA NOT NULL,
    proof BYTEA NOT NULL,
    processed BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

    PRIMARY KEY (chain_id, nonce, message_type),

    CONSTRAINT valid_message_type CHECK (
        message_type IN ('deposit', 'withdraw', 'general')
    )
);

CREATE INDEX IF NOT EXISTS idx_chain_processed ON messages (chain_id, processed);
CREATE INDEX IF NOT EXISTS idx_chain_message_type ON messages (chain_id, message_type);
CREATE INDEX IF NOT EXISTS idx_chain_nonce ON messages (chain_id, nonce);
