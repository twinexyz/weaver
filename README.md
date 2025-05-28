# weaver
A mono-repo structure for everything twine

## Prerequisities
This repo depends on the abis of twine-evm-contracts. To get the latest artifacts, run
```sh
sh scripts/generate_evm_contract_artifacts.sh
```

## Running a Twine node
```sh
cargo run --bin twine-node -- \
    node \
    --dev \
    --chain bin/node/res/dev-genesis.json \
    --http \
    --http.port 8570 \
    --ws \
    --ws.port 8571 \
    --port 8572 \
    --authrpc.port 8573 \
    --datadir /tmp/reth \
    --rpc.eth-proof-window 1000 \
    --rpc.proof-permits 1000 \
    --dev.block-time 5sec
```

## Running Apache Kafka for message transfers
We use Apache Kafka between services for message transfers. This document: (https://app.clickup.com/9018366638/docs/8crjpne-2758/8crjpne-2378) describes it in detail.

2 different `docker-compose` files have been provided:
- `kafka-1node.yml`: A single broker single controller, no HA, no replication kafka instance for local testing
- `kafka-3node.yml`: A 3 node broker + controller example instance of kafka with HA and replication. This has the ability to tolerate 1 failure of broker+controller instance.

Use any one of these via (change file according to requirement):
```sh
docker-compose -f kafka-1node.yml up
```
The kafka logs can be found in `~/.twine/docker_data_volumes/` once docker instances start on the host machine for debugging purposes.

## Running devtests
```sh
RUST_LOG=info cargo run --bin twine-devtest
```

## Contributions

### Rust Formatting
We use `rustfmt` that depends on rust nightly channel. Hence, `cargo fmt` would result in warnings and wrong formatting according to CI. Please use:
```sh
cargo +nightly fmt
```

### Pre-commit hooks
This repository makes use of `pre-commit` hooks. There are two methods of setting it up:

1. **Recommended**: A pre-downloaded artifact is provided in `artifacts/pre-commit.pyz` (originally `artifacts/pre-commit-x.y.z.pyz` and dowloaded from: [https://github.com/pre-commit/pre-commit/releases](https://github.com/pre-commit/pre-commit/releases))

2. **Optional**: Install using steps mentioned here: [https://pre-commit.com/](https://pre-commit.com/). This makes sure "pre-commit" is run every time before you try doing a `git commit`. Useful for implicit invocation.

#### Explicit invocation of hooks without `git commit`
Before every git commit, it should be ensured that following is run locally for everyone's sanity.
```
./artifacts/pre-commit.pyz run --all-files
```
