-- Add migration script here

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'on_chain_status') THEN
        CREATE TYPE on_chain_status AS ENUM (
            'pending',
            'send_failed',
            'send_successful',
            'send_failed_finalized',
            'send_successful_finalized'
        );
    END IF;
END$$;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'da_posting_status_enum') THEN
        CREATE TYPE da_posting_status_enum AS ENUM (
            'pending',
            'commit_failed',
            'committed',
            'verify_failed',
            'verified'
        );
    END IF;
END$$;

-- Track the last successfully consumed batch_id per chain (on-chain posting)
CREATE TABLE IF NOT EXISTS on_chain_progress (
    chain_id VARCHAR(50) PRIMARY KEY,
    last_consumed_batch_id BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Track the last successfully consumed batch_id per DA (verification pipeline or DA posting)
CREATE TABLE IF NOT EXISTS da_progress (
    da_id VARCHAR(50) PRIMARY KEY,
    last_consumed_batch_id BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS batches (
    batch_id BIGINT PRIMARY KEY,
    batch_hash BYTEA NOT NULL UNIQUE,

    execution_proof_data BYTEA,
    execution_proof_gen_time TIMESTAMPTZ DEFAULT now(),

    da_verification_data JSONB DEFAULT NULL,
    received_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE IF NOT EXISTS batch_status (
    batch_id BIGINT NOT NULL,
    chain_id VARCHAR(50) NOT NULL,

    batch_posted_txn TEXT DEFAULT NULL,
    on_chain_posting_status on_chain_status NOT NULL DEFAULT 'pending',
    on_chain_posting_error TEXT DEFAULT NULL,

    posted_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ DEFAULT now(),

    PRIMARY KEY (batch_id, chain_id),
    FOREIGN KEY (batch_id) REFERENCES batches(batch_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS batch_da_status (
    batch_id BIGINT NOT NULL,
    da_id VARCHAR(50) NOT NULL,

    da_verification_data JSONB DEFAULT NULL,
    da_posting_status da_posting_status_enum NOT NULL DEFAULT 'pending',
    da_posted_at TIMESTAMPTZ,
    da_verified_at TIMESTAMPTZ,

    updated_at TIMESTAMPTZ DEFAULT now(),

    PRIMARY KEY (batch_id, da_id),
    FOREIGN KEY (batch_id) REFERENCES batches(batch_id) ON DELETE CASCADE
);
