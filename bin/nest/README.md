# NEST sequencer

## Running the sequencer
The sequencer runs in two different modes,
1. Sequencing mode
In this mode, the sequencer connects to the Executation Layer and progresses the block.
```bash
cargo run --release --bin twine-nest --no-default-features --features sequencer -- --config bin/nest/config/sequencer.yaml
```

2. Verifier mode
In this mode, the sequencer verifies the state consistency of the underlying L1 chains against the actual twine state
```bash
cargo run --release --bin twine-nest --no-default-features --features verifier -- --config bin/nest/config/sequencer.yaml
```

3. Combined mode
In this mode, the sequencer does both sequencing and verifying job
```bash
cargo run --release --bin twine-nest -- --config bin/nest/config/sequencer.yaml
```
