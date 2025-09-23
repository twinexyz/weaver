#!/bin/bash

rm -rf /tmp/solana/

echo "Starting Solana node..."

solana-test-validator \
  --reset \
  --ledger /tmp/solana &
