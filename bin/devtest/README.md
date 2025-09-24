# Twine Devtest

`twine-devtest` is a lightweight harness for running a small set of manual integration checks against a live Twine node.

## Prerequisites

- Rust toolchain installed.
- A Twine node (or compatible JSON-RPC endpoint) listening on `http://127.0.0.1:8545`. The bundled tests assume this port and will fail otherwise. You can run the node with [this](../../README.md#running-a-twine-node) command


## Running tests

Run all registered tests:

```bash
RUST_LOG=info cargo run --bin twine-devtest
```

Filter tests using a regex (e.g., only run the precompile test):

```bash
RUST_LOG=info cargo run --bin twine-devtest -- precompile
```

The binary will execute each matching test in sequence and log results to stdout.

## Adding tests

Tests live under `bin/devtest/src`. Register additional checks in `tests.rs` and implement the logic in a dedicated module so they can be picked up by the harness.
