# TWINE PROOF SCHEDULER

Twine Proof Scheduler schedules the execution proof of twine node and consensus proof of solana l1 chain. It accepts the incoming workers and provides them proving jobs. It accepts the job's results from the provers and posts the proofs in a kafka queue for the aggregator to consume.



## Running the Scheduler
```sh
# Run L2 execution proof scheduler
RUST_LOG=info cargo run --release --bin twine-proof-scheduler-bin --features l2-proof-scheduler -- --config l2-scheduler-config.toml
# Run solana execution proof scheduler
RUST_LOG=info cargo run --release --bin twine-proof-scheduler-bin --features solana-proof-scheduler -- --config solana-scheduler-config.toml
```

## Prerequisits
### Default setup
(From the base dir)
1. Start twine node in port `8545`

    [TWINE-DOC](../../README.md)

2. Start local postgres instance, kafka instance
```bash
cd docker
docker compose up -d
```
3. Rename config
```bash
# Prepare configuration for L2 proof scheduler
mv example.l2-scheduler-config.toml l2-scheduler-config.toml
# Prepare configuration for solana proof scheduler
mv example.solana-scheduler-config.toml solana-scheduler-config.toml
```
4. Run scheduler binary

### Custom setup (L2 proof scheduler)
The example configuration can be found in the base directory.
1. Replace twine node url in the `example.l2-scheduler-config.toml`
```
twine_rpc_url = "http://127.0.0.1:8545
```
2. Configure Database:
Postgres Database is used to store various states of scheduler.
replace in the `example.l2-scheduler-config.toml`
```
conn_str = "postgres://postgres:postgres@localhost:5432/l2-scheduler"
```
with your connection string.

3. Configure kafka:
Kafka queue is used to post the proofs so that aggregator can consume them.
replace in the `example.l2-scheduler-config.toml`

```
kafka_broker_url = "localhost:9092"
kafka_topics = "demo"
kafka_groups = "my-group"
auto_offset_reset = "earliest"
```
with your setup.

Also replace all the other relevant configurations.

4. Rename config
```sh
mv example.l2-scheduler-config.toml l2-scheduler-config.toml
```
5. Run the scheduler binary


### Custom setup (L2 proof scheduler)
The example configuration can be found in the base directory.
1. Replace twine node url in the `example.solana-scheduler-config.toml`
```
twine_rpc_url = "http://127.0.0.1:8545
```
2. Configure Database:
Postgres Database is used to store various states of scheduler.
replace in the `example.solana-scheduler-config.toml`
```
conn_str = "postgres://postgres:postgres@localhost:5432/solana-scheduler"
```
with your connection string.

3. Configure kafka:
Kafka queue is used to post the proofs so that aggregator can consume them.
replace in the `example.solana-scheduler-config.toml`

```
kafka_broker_url = "localhost:9092"
kafka_topics = "demo"
kafka_groups = "my-group"
auto_offset_reset = "earliest"
```
with your setup.

Also replace all the other relevant configurations.

4. Rename config
```sh
mv example.solana-scheduler-config.toml solana-scheduler-config.toml
```
5. Run the scheduler binary
