#! /bin/bash
set -euo pipefail

RPC_URL="${RPC_URL:-http://127.0.0.1:8545}"
PK="${PK:-0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d}"
ADDR="${ADDR:-0x70997970C51812dc3A010C7d01b50e0d17dc79C8}"
COUNT="${COUNT:-20}"
DEPOSIT_TO="${DEPOSIT_TO:-0x23618e81E3f5cdF7f54C3d65f7FBc0aBf5B21E8f}"
AMOUNT="${AMOUNT:-10}"
ARG3="${ARG3:-0}"

# Gateway contract needed
: "${GATEWAY:?Set GATEWAY env var to the contract address}"

echo "RPC_URL=$RPC_URL"
echo "From ADDR=$ADDR"
echo "GATEWAY=$GATEWAY"
echo "COUNT=$COUNT"

# Get starting nonce for ADDR
START_NONCE="$(cast nonce "$ADDR" --rpc-url "$RPC_URL")"
echo "[batch_deposit] Starting nonce: $START_NONCE"

# Fire COUNT transactions in parallel without waiting for receipts
for (( i=0; i<COUNT; i++ )); do
  NONCE=$(( START_NONCE + i ))
  (
    echo "[batch_deposit] Sending tx #$i (nonce=$NONCE)..."
    cast send "$GATEWAY" \
      "depositETH(address,uint256,uint256)" \
      "$DEPOSIT_TO" "$AMOUNT" "$ARG3" \
      --gas-limit 1000000 \
      --value "$AMOUNT" \
      --rpc-url "$RPC_URL" \
      --private-key "$PK" \
      --nonce "$NONCE" \
      --async
  ) &
done

# Wait for all background sends to be issued
wait
echo "[batch_deposit] Dispatched $COUNT transactions."
