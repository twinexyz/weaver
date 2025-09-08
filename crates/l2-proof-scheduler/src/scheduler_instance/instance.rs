//! Proof scheduler instance
#![allow(unused_imports)]

use std::time::Duration;

use orchestrator_rs::database::inmem::InMemoryDatabase;
use orchestrator_rs::database::postgresql::PostgresDatabase;
use orchestrator_rs::instance::instance::Instance;
use orchestrator_rs::instrumentation::dummy_instrumentation::DummyInstrumentation;
use orchestrator_rs::instrumentation::open_telemetry::Prometheus;
use orchestrator_rs::processor::dynamic_processor::DynamicProcessor;
use orchestrator_rs::processor::simple_processor::SimpleProcessor;
use orchestrator_rs::transform::TransformRequest;
use orchestrator_rs::worker::worker_manager::WorkerManager;
use sqlx::postgres::PgPoolOptions;

use crate::batch_subscriber::TwineBatchSubscriber;
use crate::batch_transform::transform_attempt::{
    TwineBatchTransformAttempt, TwineBatchTransformAttemptID, TwineBatchTransformReturnType,
};
use crate::batch_transform::transform_attempt_creator::TwineBatchTransformAttemptCreator;
use crate::batch_transform::transform_request::{
    TwineBatchTransformInput, TwineBatchTransformRequest, TwineBatchTransformRequestID,
};
use crate::config::TwineProofSchedulerConfig;
use crate::consumer::consume_attempt::{
    TwineBatchTransformResultConsumeAttempt, TwineBatchTransformResultConsumeAttemptID,
};
use crate::consumer::consume_attempt_creator::TwineBatchTransformResultConsumeAttemptCreator;
use crate::consumer::consumer::TwineBatchTransformResultConsumer;
use crate::worker_manager::manager::TwineWorkerManager;

/// Proof scheduler instance
#[derive(Debug)]
pub struct TwineProofSchedulerInstance {}

impl Instance for TwineProofSchedulerInstance {
    type Config = TwineProofSchedulerConfig;
    type ConsumeAttempt = TwineBatchTransformResultConsumeAttempt;
    type ConsumeAttemptCreator = TwineBatchTransformResultConsumeAttemptCreator;
    type ConsumeAttemptIdentifier = TwineBatchTransformResultConsumeAttemptID;
    type Consumer = TwineBatchTransformResultConsumer;
    type Database = PostgresDatabase<
        TwineBatchTransformRequest,
        TwineBatchTransformAttempt,
        TwineBatchTransformResultConsumeAttempt,
        TwineProofSchedulerConfig,
    >;
    type Emitter = TwineBatchSubscriber;
    type Input = TwineBatchTransformInput;
    type Instrumentation = Prometheus<
        TwineBatchTransformRequest,
        TwineBatchTransformAttempt,
        TwineBatchTransformResultConsumeAttempt,
        Self::Config,
    >;
    type Output = TwineBatchTransformReturnType;
    type Processor = DynamicProcessor<
        TwineProofSchedulerConfig,
        TwineBatchTransformRequest,
        TwineBatchTransformAttempt,
        TwineBatchTransformAttemptCreator,
        TwineBatchTransformResultConsumeAttempt,
        TwineBatchTransformResultConsumeAttemptCreator,
        Self::Database,
        Self::Instrumentation,
    >;
    type StaticConfigHandle = String;
    type TransformAttempt = TwineBatchTransformAttempt;
    type TransformAttemptCreator = TwineBatchTransformAttemptCreator;
    type TransformAttemptIdentifier = TwineBatchTransformAttemptID;
    type TransformRequest = TwineBatchTransformRequest;
    type TransformRequestIdentifier = TwineBatchTransformRequestID;
    type WorkerManager = TwineWorkerManager;
}
