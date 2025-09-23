#!/bin/bash

rm -rf /tmp/reth/

echo "Starting Reth node..."
reth node \
  --dev \
  --http --http.port 8570 \
  --ws --ws.port 8571 \
  --port 8572 \
  --authrpc.port 8573 \
  --datadir /tmp/reth \
  --rpc.eth-proof-window 1000 \
  --rpc.proof-permits 1000 \
  --dev.block-time 5sec &