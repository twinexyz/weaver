# Twine Execution Prover Worker

This generates the execution proof of twine node. It internally uses [rsp](https://github.com/twinexyz/twine-rsp) for generating the proof.

## Prerequisits
1. Install [rsp](https://github.com/twinexyz/twine-rsp) binary
2. Run l2 proof scheduler [docs](../l2-proof-scheduler-bin/README.md)
3. The default ws server to connect to the scheduler runs in port 8000, if not fix it accordingly
4. Run the worker

## Running the worker
Worker can be run in different modes,
1. Dummy Mode: Proof is not produced in this mode
```bash
RUST_LOG=info cargo run --release --bin twine-l2-execution-prover-worker -- --worker-manager-url ws://0.0.0.0:8000 --genesis-path <path_to_twine's_genesis file>
```
2. Proving Mode: Proof is produced in this mode
```bash
RUST_LOG=info cargo run --release --bin twine-l2-execution-prover-worker -- --worker-manager-url ws://0.0.0.0:8000 --prove --sp1-port <port> --runtime-env <docker_if_running_in_docker__empty otherwise> --network <network> --genesis-path <path_to_twine's_genesis file>
```
