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
use orchestrator_rs::transform::{TransformAttempt, TransformAttemptCreator, TransformRequest};
use orchestrator_rs::worker::worker_manager::WorkerManager;
use sqlx::postgres::PgPoolOptions;
use twine_proof_scheduler_common::config::ProofSchedulerConfig;
use twine_proof_scheduler_common::error::ProofSchedulerError;

use crate::consumer::consume_attempt::{
    SolanaMessageConsumeAttempt, SolanaMessageConsumeAttemptID,
};
use crate::consumer::consume_attempt_creator::SolanaMessageTransformResultConsumeAttemptCreator;
use crate::consumer::consumer::SolanaProofConsumer;
use crate::l1_subscriber::SolanaMessageSubscriber;
use crate::message_transform::message_transform_attempt::{
    SolanaMessageTransformAttempt, SolanaMessageTransformAttemptID,
    SolanaMessageTransformReturnType,
};
use crate::message_transform::message_transform_attempt_creator::SolanaMessageTransformAttemptCreator;
use crate::message_transform::message_transform_request::{
    SolanaMessageTransformInput, SolanaMessageTransformRequest, SolanaMessageTransformRequestID,
};
use crate::worker_manager::manager::SolanaProverWorkerManager;

/// Proof scheduler instance
#[derive(Debug)]
pub struct SolanaProofSchedulerInstance {}

impl Instance for SolanaProofSchedulerInstance {
    type Config = ProofSchedulerConfig;
    type ConsumeAttempt = SolanaMessageConsumeAttempt;
    type ConsumeAttemptCreator = SolanaMessageTransformResultConsumeAttemptCreator;
    type ConsumeAttemptIdentifier = SolanaMessageConsumeAttemptID;
    type Consumer = SolanaProofConsumer;
    type Database = PostgresDatabase<
        SolanaMessageTransformRequest,
        SolanaMessageTransformAttempt,
        SolanaMessageConsumeAttempt,
        ProofSchedulerConfig,
    >;
    type Emitter = SolanaMessageSubscriber;
    type Input = SolanaMessageTransformInput;
    type Instrumentation = Prometheus<
        SolanaMessageTransformRequest,
        SolanaMessageTransformAttempt,
        SolanaMessageConsumeAttempt,
        Self::Config,
    >;
    type Output = SolanaMessageTransformReturnType;
    type Processor = DynamicProcessor<
        ProofSchedulerConfig,
        SolanaMessageTransformRequest,
        SolanaMessageTransformAttempt,
        SolanaMessageTransformAttemptCreator,
        SolanaMessageConsumeAttempt,
        SolanaMessageTransformResultConsumeAttemptCreator,
        Self::Database,
        Self::Instrumentation,
    >;
    type StaticConfigHandle = String;
    type TransformAttempt = SolanaMessageTransformAttempt;
    type TransformAttemptCreator = SolanaMessageTransformAttemptCreator;
    type TransformAttemptIdentifier = SolanaMessageTransformAttemptID;
    type TransformRequest = SolanaMessageTransformRequest;
    type TransformRequestIdentifier = SolanaMessageTransformRequestID;
    type WorkerManager = SolanaProverWorkerManager;
}
