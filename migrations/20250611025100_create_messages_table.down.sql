-- Drop indexes before dropping the table
DROP INDEX IF EXISTS idx_chain_nonce;
DROP INDEX IF EXISTS idx_chain_message_type;
DROP INDEX IF EXISTS idx_chain_processed;

DO $$
BEGIN
    IF current_setting('app.env', true) = 'dev' THEN
        DROP TABLE IF EXISTS messages;
    ELSIF NOT EXISTS (SELECT 1 FROM messages LIMIT 1) THEN
        DROP TABLE IF EXISTS messages;
    ELSE
        RAISE NOTICE 'Skipping drop of messages — table not empty and environment is not dev.';
    END IF;
END$$;
