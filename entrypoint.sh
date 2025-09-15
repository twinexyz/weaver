#!/bin/bash

MODE=$1
CONFIG_FILE=$1

# Check if the config file is provided
if [ -z "$CONFIG_FILE" ]; then
  echo "No config file provided. Exiting."
  exit 1
fi

# Convert JSON to YAML
yq eval -o=yaml /config.json > "$CONFIG_FILE"

# Run the scheduler with the provided config file
RUST_LOG=info /usr/local/bin/$MODE --config "$CONFIG_FILE" run
