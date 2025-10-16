-- Drop indexes first
DROP INDEX IF EXISTS idx_withdrawal_events_event_type;
DROP INDEX IF EXISTS idx_withdrawal_events_l1_chain_id;
DROP INDEX IF EXISTS idx_withdrawal_events_is_failed;
DROP INDEX IF EXISTS idx_withdrawal_events_is_processed;
DROP INDEX IF EXISTS idx_withdrawal_events_l2_txn_hash;

-- Drop the table
DROP TABLE IF EXISTS withdrawal_events;

-- Drop the enum type
DROP TYPE IF EXISTS withdrawal_event_type;
