#!/bin/bash
   
rm -rf /tmp/twine/

echo "Starting Twine node..."

# FIXME: Hardcoded path, have to overwrite the genesis file 
# from the test itself. 
twine-node node \
  --chain /home/nobel/dev/weaver/bin/node/res/dev-genesis.json \
  --dev \
  --http --http.port 8545 \
  --datadir /tmp/twine \
  --rpc.eth-proof-window 1000 \
  --rpc.proof-permits 1000 \
  --ws \
  --dev.block-time 5sec &