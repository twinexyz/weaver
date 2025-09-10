#!/usr/bin/env bash
set -e

# This runs only on first init (empty PGDATA).
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
  -- Database 1 + user
  CREATE DATABASE scheduler;
  GRANT ALL PRIVILEGES ON DATABASE scheduler TO postgres;

  -- Database 2 + user
  CREATE DATABASE aggregator;
  GRANT ALL PRIVILEGES ON DATABASE aggregator TO postgres;

  -- Database 2 + user
  CREATE DATABASE merkora;
  GRANT ALL PRIVILEGES ON DATABASE merkora TO postgres;
EOSQL
