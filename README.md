# weaver
A mono-repo structure for everything twine

## Prerequisities
This repo depends on the abis of twine-evm-contracts. To get the latest artifacts, run
```sh
sh scripts/generate_evm_contract_artifacts.sh
```

## Running a Twine node
```sh
cargo run --release \
    --bin twine-node -- \
    node \
    --dev \
    --chain bin/node/res/local-genesis.json \
    --http \
    --ws \
    --datadir /tmp/reth \
    --rpc.eth-proof-window 1000 \
    --rpc.proof-permits 1000 \
    --dev.block-time 5sec
```

## Running devtests
```sh
RUST_LOG=info cargo run --bin twine-devtest
```

## Running integration test cases
Integration cases are at the `crates/integration-tests/tests` directory. Before running these make sure you have the correct config set at
`crates/integration-tests/res/integration_test_config.yaml` file. Then any specific test case be run as
```sh
RUST_LOG=info cargo test -p twine-integration-tests --test file_name  test_name-- --show-output
```
For example:
```sh
RUST_LOG=info cargo test -p twine-integration-tests --test eth_deposit test_deposit -- --show-output
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
