# TWINE PROOF SCHEDULER

Twine Proof Scheduler schedules the execution proof of twine node. It accepts the incoming workers and provides them proving jobs. It accepts the job's results from the provers and posts the proofs in a kafka queue for the aggregator to consume.



## Running the Scheduler 
```sh
RUST_LOG=info cargo run --release --bin twine-l2-proof-scheduler-bin
```

## Prerequisits
### Default setup 
(From the base dir)
1. Start twine node in port `8545`

    [TWINE-DOC](../../README.md)

2. Start local postgres instance
```bash
docker compose up -d 
```
3. Start local kafka instance 
```bash 
docker compose -f kafka-compose-yaml up -d
```
4. Rename config 
```bash 
mv example.scheduler-config.toml config.toml
```
4. Run scheduler binary 

### Custom setup 
The example configuration can be found in the base directory. 
1. Replace twine node url in the `example.scheduler-config.toml`
```
twine_rpc_url = "http://127.0.0.1:8545
```
2. Configure Database:
Postgres Database is used to store various states of scheduler. 
replace in the `example.scheduler-config.toml`
```
conn_str = "postgres://twine:twine@localhost:5432/twine"
```
with your connection string. 
3. Configure kafka: 
Kafka queue is used to post the proofs so that aggregator can consume them. 
replace in the `example.scheduler-config.toml`

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
mv example.scheduler-config.toml config.toml
```
5. Run the scheduler binary

