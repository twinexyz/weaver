#!/bin/bash

# Function to display usage information
usage() {
    echo "Usage: $0 [--holesky | --sepolia] --height=<HEIGHT>"
    exit 1
}

# Default variables
NETWORK=""
HEIGHT=""

# API keys for each network (replace with your actual API keys)
HOLESKY_API_KEY=""
SEPOLIA_API_KEY=""

# Parse command-line arguments
for arg in "$@"; do
    case $arg in
        --holesky)
            NETWORK="holesky"
            ;;
        --sepolia)
            NETWORK="sepolia"
            ;;
        --height=*)
            HEIGHT="${arg#*=}"
            ;;
        *)
            echo "Invalid argument: $arg"
            usage
            ;;
    esac
done

# Validate arguments
if [[ -z "$NETWORK" || -z "$HEIGHT" ]]; then
    echo "Error: Both network and height must be specified."
    usage
fi

# Set the correct URL and API key based on the network
if [[ "$NETWORK" == "holesky" ]]; then
    BASE_URL="https://ethereum-holesky.core.chainstack.com"
    API_KEY="$HOLESKY_API_KEY"
elif [[ "$NETWORK" == "sepolia" ]]; then
    BASE_URL="https://ethereum-sepolia.core.chainstack.com"
    API_KEY="$SEPOLIA_API_KEY"
    # BASE_URL="http://k8s-kttestne-testnetc-fb570bb280-1376030216.us-east-2.elb.amazonaws.com"
    # API_KEY="$SEPOLIA_API_KEY"
else
    echo "Error: Unknown network."
    usage
fi

# Execute the curl command
echo "Fetching updates for $NETWORK at height $HEIGHT..."
curl -X 'GET' \
  "$BASE_URL/beacon/${API_KEY}/eth/v1/beacon/light_client/updates?start_period=$HEIGHT&count=1" \
  -H 'accept: application/json' > "${NETWORK}_updates.json"

echo "Updates saved to ${NETWORK}_updates.json"
