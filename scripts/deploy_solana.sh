#!/usr/bin/env bash
set -euo pipefail

echo "Starting Solana node..."

if [[ -n "${SOLANA_DATA_DIR:-}" ]]; then
    DATA_DIR="$SOLANA_DATA_DIR"
else
    DATA_DIR="/tmp/solana"
fi

if [[ -n "${SOLANA_BIN:-}" ]]; then
    BIN="$SOLANA_BIN"
else
    BIN="solana-test-validator"
fi

"$BIN" \
  --reset \
  --ledger "$DATA_DIR" &
