#!/usr/bin/env bash
set -euo pipefail

echo "Starting Twine node..."

# Genesis file: from $TWINE_GENESIS_FILE or fallback to parent/bin/node/res/local-genesis.json
if [[ -n "${TWINE_GENESIS_FILE:-}" ]]; then
    GENESIS="$TWINE_GENESIS_FILE"
else
    SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
    PARENT_DIR="$(dirname "$SCRIPT_DIR")"
    GENESIS="$PARENT_DIR/bin/node/res/local-genesis.json"
fi

# Data directory: from $TWINE_DATA_DIR or fallback to /tmp/twine
if [[ -n "${TWINE_DATA_DIR:-}" ]]; then
    DATA_DIR="$TWINE_DATA_DIR"
else
    DATA_DIR="/tmp/twine"
fi

if [[ -n "${TWINE_BIN:-}" ]]; then
    BIN="$TWINE_BIN"
else
    BIN="twine-node"
fi

echo "Using genesis file: $GENESIS"
echo "Using data directory: $DATA_DIR"

"$BIN" node \
  --chain "$GENESIS" \
  --dev \
  --http --http.port 8545 \
  --datadir "$DATA_DIR" \
  --rpc.eth-proof-window 1000 \
  --rpc.proof-permits 1000 \
  --ws \
  --dev.block-time 5sec \
  --twine.max-blocks-per-batch 5 &
