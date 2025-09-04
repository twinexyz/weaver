# Aggregator

The Aggregator is a core component of the Twine network that collects and processes data from multiple sources, validates proofs, and submits them to settlement chains.

## Overview

The Aggregator performs the following key functions:
- Polls the Twine chain for new batches
- Consumes proofs from Kafka topics (primary method)
- Validates execution proofs
- Dispatches validated proofs to settlement chains (Ethereum, Solana, etc.)
- Optionally posts data to Data Availability layers
- Validates data posted to DA is stored on L1s

## Prerequisites

Before running the aggregator, ensure you have:
- Rust toolchain installed
- PostgreSQL database running
- Access to configured blockchain RPC endpoints
- Kafka cluster running (if using Kafka integration)
- Required environment variables set

## Configuration

The aggregator requires a YAML configuration file. By default, it looks for `config.yaml` in the current directory, but you can specify a different path using the `--config` flag.

Example configuration can be found [here](./res/config.yaml)

## Running the Aggregator

To run the aggregator, use the following command:

```bash
cargo run --bin aggregator -- run
```

To specify a custom configuration file:

```bash
cargo run --bin aggregator -- --config /path/to/config.yaml run
```

## Commands

The aggregator supports several commands:

- `run`: Start the aggregator service
- `genesis`: Initialize genesis state
- `show-config`: Display the current configuration

## Features

### Data Availability
When `dispatcher.use_da` is set to `true`, the aggregator will post data to a Data Availability layer (currently Celestia).

### Multi-chain Settlement
The aggregator can settle proofs to multiple target chains:
- Ethereum
- Solana

### Kafka Integration
Proofs can be consumed from Kafka topics, allowing for scalable and distributed proof processing.

### JSON RPC Server (Alternative to Kafka)
In case Kafka is not available or as an alternative method, the aggregator also exposes a JSON RPC server for submitting proofs directly. This server is configured through the `rpc` section in the configuration file.

The RPC server exposes the following methods:
- `twagg_submitProof`: Submit a ZK proof for processing
- `twagg_health`: Health check endpoint that returns "ok" when the server is running

<details>
<summary>Example: Submitting a proof via RPC</summary>

crates/types/src/proofs.rs


```bash
curl -X POST \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "twagg_submitProof",
    "params": [{
      // ZkProof
    }],
    "id": 1
  }' \
  http://127.0.0.1:5566
```
</details>
<details>
<summary>Example: Health check endpoint</summary>

```bash
curl -X POST \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "twagg_health",
    "params": [],
    "id": 1
  }' \
  http://127.0.0.1:5566
```
</details>

## Monitoring

The aggregator exposes metrics via a Prometheus-compatible endpoint when `telemetry.metrics_server` is configured.

## Logging

Logs are written to `twine_aggregator.log` in the current directory. The log level can be controlled through the standard Rust logging environment variables.
