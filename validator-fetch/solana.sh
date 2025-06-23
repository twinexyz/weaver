#!/bin/bash

set -e

RPC_URL="https://api.mainnet-beta.solana.com"

TARGET_SLOT=$1
OUTPUT_FILE="./proof_data/validator_set.json"

# Check if TARGET_SLOT is provided
if [ -z "$TARGET_SLOT" ]; then
    echo "Usage: $0 <target_slot>"
    exit 1
fi

echo "Fetching epoch information..."
EPOCH_INFO=$(solana epoch-info --url $RPC_URL --output json)

if [ -z "$EPOCH_INFO" ]; then
    echo "Failed to fetch epoch info. Ensure solana CLI is installed and $RPC_URL is accessible."
    exit 1
fi

SLOTS_IN_EPOCH=$(echo $EPOCH_INFO | jq -r '.slotsInEpoch')
ABSOLUTE_SLOT=$(echo $EPOCH_INFO | jq -r '.absoluteSlot')
CURRENT_EPOCH=$(echo $EPOCH_INFO | jq -r '.epoch')

if [ -z "$SLOTS_IN_EPOCH" ] || [ -z "$ABSOLUTE_SLOT" ] || [ -z "$CURRENT_EPOCH" ]; then
    echo "Failed to parse epoch info. Ensure solana CLI and jq are correctly set up."
    exit 1
fi

TARGET_EPOCH=$(( TARGET_SLOT / SLOTS_IN_EPOCH ))

echo "Target slot $TARGET_SLOT belongs to epoch $TARGET_EPOCH."

echo "Fetching validator set..."
VALIDATOR_SET=$(solana validators --url $RPC_URL --output json)

if [ -z "$VALIDATOR_SET" ]; then
    echo "Failed to fetch validator set."
    exit 1
fi

VALIDATOR_INFO=$(echo $VALIDATOR_SET | jq --arg EPOCH "$TARGET_EPOCH" '{
    epoch: $EPOCH,
    validators: [.validators[] | {
        identity_pubkey: .identityPubkey,
        vote_account_pubkey: .voteAccountPubkey,
        stake: .activatedStake,
        commission: .commission
    }]
}')

echo "Saving validator set to $OUTPUT_FILE..."
echo "$VALIDATOR_INFO" | jq '.' > "$OUTPUT_FILE"

echo "Validator set for epoch $TARGET_EPOCH saved to $OUTPUT_FILE."
