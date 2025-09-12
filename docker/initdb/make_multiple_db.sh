#!/usr/bin/env bash
set -e

# This runs only on first init (empty PGDATA).
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
  -- Database 1 + user
  CREATE DATABASE solana-scheduler;
  GRANT ALL PRIVILEGES ON DATABASE solana-scheduler TO postgres;

  -- Database 2 + user
  CREATE DATABASE l2-scheduler;
  GRANT ALL PRIVILEGES ON DATABASE l2-scheduler TO postgres;

  -- Database 3 + user
  CREATE DATABASE aggregator;
  GRANT ALL PRIVILEGES ON DATABASE aggregator TO postgres;

  -- Database 4 + user
  CREATE DATABASE merkora;
  GRANT ALL PRIVILEGES ON DATABASE merkora TO postgres;
EOSQL
