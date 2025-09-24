#!/usr/bin/env bash
set -euo pipefail

echo "Starting Reth node..."

if [[ -n "${RETH_DATA_DIR:-}" ]]; then
    DATA_DIR="$RETH_DATA_DIR"
else
    DATA_DIR="/tmp/reth"
fi

reth node \
  --dev \
  --http --http.port 8570 \
  --ws --ws.port 8571 \
  --port 8572 \
  --authrpc.port 8573 \
  --datadir "$DATA_DIR" \
  --rpc.eth-proof-window 1000 \
  --rpc.proof-permits 1000 \
  --dev.block-time 5sec &
