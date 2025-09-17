#!/bin/bash

MODE=$1
CONFIG_FILE=$2

# Check if both arguments are provided
if [ -z "$MODE" ] || [ -z "$CONFIG_FILE" ]; then
  echo "Usage: $0 <mode> <config_file.yaml|toml>"
  exit 1
fi

# Detect extension and convert JSON accordingly
case "$CONFIG_FILE" in
  *.yaml|*.yml)
    echo "Converting JSON to YAML..."
    yq -p json -o yaml /config.json > "$CONFIG_FILE"
    ;;
  *.toml)
    echo "Converting JSON to TOML..."
    cat /config.json | tomq -T > "$CONFIG_FILE"
    ;;
  *)
    echo "Unsupported config file format. Please use .yaml or .toml"
    exit 1
    ;;
esac

# Run the scheduler with the provided config file
RUST_LOG=info /usr/local/bin/"$MODE" --config "$CONFIG_FILE" run
