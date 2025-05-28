use std::time::Duration;

use log::{info, warn};
use rdkafka::config::RDKafkaLogLevel;
use rdkafka::consumer::{Consumer, ConsumerContext, StreamConsumer};
use rdkafka::error::KafkaResult;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::util::get_rdkafka_version;
use rdkafka::{ClientConfig, ClientContext, Message, TopicPartitionList};

// A simple context to customize the consumer behavior and print a log line
// every time offsets are committed
struct LoggingConsumerContext;

impl ClientContext for LoggingConsumerContext {}

impl ConsumerContext for LoggingConsumerContext {
    fn commit_callback(&self, result: KafkaResult<()>, _offsets: &TopicPartitionList) {
        match result {
            Ok(_) => info!("Offsets committed successfully"),
            Err(e) => warn!("Error while committing offsets: {}", e),
        };
    }
}
// Define a new type for convenience
type LoggingConsumer = StreamConsumer<LoggingConsumerContext>;

#[tokio::main]
async fn main() {
    env_logger::init();
    let (_, version) = get_rdkafka_version();
    info!("rd_kafka_version: {}", version);

    // Comma separated brokers
    let brokers = "0.0.0.0:19092";

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .set("queue.buffering.max.ms", "0") // Do not buffer
        .create()
        .expect("Producer creation failed");

    let pre_send_time = std::time::Instant::now();
    let send_msg_count = 10_000;
    for i in 0..send_msg_count {
        if i % 1000 == 0 {
            info!("{} messages sent", i);
        }
        let mut record = FutureRecord::to("test-topic");
        record = record.payload("test message");
        record = record.key("test key");

        producer
            .send(record, Duration::from_secs(5))
            .await
            .expect("Failed to wait for message send");
    }
    let elapsed = pre_send_time.elapsed();
    info!(
        "Sent {} messages in {:?}, {:?}/op",
        send_msg_count,
        elapsed,
        elapsed / send_msg_count
    );

    // Create a consumer to ensure the messages are sent correctly.
    let consumer: LoggingConsumer = ClientConfig::new()
        .set("group.id", "test-message-consumer-group")
        .set("bootstrap.servers", brokers)
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "6000")
        // Commit automatically every 5 seconds.
        .set("enable.auto.commit", "true")
        .set("auto.commit.interval.ms", "5000")
        // but only commit the offsets explicitly stored via `consumer.store_offset`.
        .set("enable.auto.offset.store", "false")
        .set_log_level(RDKafkaLogLevel::Debug)
        .create_with_context(LoggingConsumerContext)
        .expect("Consumer creation failed");

    consumer
        .subscribe(&["test-topic"])
        .expect("Can't subscribe to specified topic");

    info!("Waiting for messages...");
    let mut message_count: u32 = 0;
    let pre_receive_time = std::time::Instant::now();
    match consumer.recv().await {
        Ok(message) => {
            message_count += 1;
            let offset = message.offset();
            info!("Received message with offset: {}", offset);
            if message_count % 1000 == 0 {
                info!("{} messages received", message_count);
            }
            // Now that the message is completely processed, add it's position to the offset
            // store. The actual offset will be committed every 5 seconds.
            if let Err(e) = consumer.store_offset_from_message(&message) {
                warn!("Error while storing offset: {}", e);
            }
        }
        Err(e) => {
            warn!("Error while receiving message: {}", e);
        }
    }
    let elapsed = pre_receive_time.elapsed();
    info!(
        "Received {} messages in {:?}, {:?}/op",
        message_count,
        elapsed,
        elapsed / message_count
    );
}
