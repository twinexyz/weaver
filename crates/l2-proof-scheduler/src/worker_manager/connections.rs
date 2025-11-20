//! manages connection with the worker managers
use std::collections::HashMap;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use orchestrator_rs::worker::worker_manager::WorkerManagerResult;
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use tokio::time::Instant;
use tokio_tungstenite;
use twine_proof_scheduler_common::error::ProofSchedulerError;

use crate::batch_transform::transform_attempt::{
    TwineBatchTransformAttempt, TwineBatchTransformAttemptID, TwineBatchTransformReturnCtx,
};
use crate::batch_transform::transform_request::TwineBatchTransformRequestID;

/// Connection Message Types
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ConnectionMessageTypes {
    /// Worker Instances requests new jobs via `NewJob` message type
    NewJob,
    /// Result of the job is sent via `JobResult` message type
    JobResult,
    /// Invalid params
    InvalidParams,
    /// Message not ready
    MessageNotReady,
    /// Keep alive
    KeepAlive,
}

/// Message structure sent by the worker instances
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConnectionMessage {
    /// Message type to request new job or send result of the
    /// previously taken job by the worker instances
    pub message_type: ConnectionMessageTypes,
    /// Message data
    pub message: MessageData,
}

impl ConnectionMessage {
    /// default connection message with type
    pub fn default_message_with_type(connection_message_type: ConnectionMessageTypes) -> Self {
        Self {
            message_type: connection_message_type,
            message: MessageData {
                transform_attempt_id: TwineBatchTransformAttemptID {
                    identifier: 0,
                    transform_request_id: TwineBatchTransformRequestID { identifier: 0 },
                },
                data: String::new(),
            },
        }
    }
}

/// Message data
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MessageData {
    /// Represents the message associated to the Transform Attempt
    pub transform_attempt_id: TwineBatchTransformAttemptID,
    /// json serialized message
    pub data: String,
}

/// Connection ID
///
/// Each connection to the worker instances is uniquely identified by this
/// connection ID, Worker instances cannot send the job result outside this
/// connection ID. i.e. If the worker instance reconnects, the job result
/// of the workers before the disconnect event is not accepted by the
/// manager
#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct ConnectionID(pub u64);

/// Keeps track of the assigned jobs to the connected worker instances
#[derive(Debug)]
pub struct Connections {
    /// workers assignment and connection details
    pub worker_connection_details: Mutex<WorkerAssignmentConnectionDetails>,
    /// job completion timeout
    pub job_completion_timeout: u64,
    /// last timed out job checked
    pub last_timed_out_check: Mutex<Instant>,
}

/// keeps track of the workers connection and jobs
#[derive(Debug)]
pub struct WorkerAssignmentConnectionDetails {
    /// hashmap to store the assigned jobs to the worker instances
    pub assigned_jobs: HashMap<(TwineBatchTransformAttemptID, ConnectionID), JobDetails>,
    /// keeps track of the existing connections to the worker instances
    pub connection_status: HashMap<ConnectionID, ConnectionStatus>,
}

/// Connection status of the worker instances
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    /// healty connection
    Healthy,
    /// disconnected to the workers
    Disconnected,
}

/// Structure to represent the details of the assigned jobs to
/// the worker instances
#[derive(Debug, Clone)]
pub struct JobDetails {
    /// transform attempt
    pub transform_attempt: TwineBatchTransformAttempt,
    /// time of assignment
    /// each job can have a specific timeout period, if the job result
    /// exceeds this timeout period, the connection to that worker is
    /// removed and the transform reattempt is made via the processor
    pub assigned_at: Instant,
}

impl Connections {
    /// accepts the incoming connection from the worker instances and handles
    /// them according to the connection message
    pub async fn accept_connection(
        &self,
        connection_id: Arc<Mutex<ConnectionID>>,
        stream: TcpStream,
        _sender: Sender<WorkerManagerResult<TwineBatchTransformAttempt>>,
        job_mutex: Arc<Mutex<Option<TwineBatchTransformAttempt>>>,
    ) {
        let addr = stream
            .peer_addr()
            .expect("connected streams should have a peer address");
        log::info!("Peer address: {addr}");

        let ws_stream = tokio_tungstenite::accept_async(stream)
            .await
            .expect("Error during the websocket handshake occurred");

        log::info!("New WebSocket connection: {addr}");
        let mut connection_id = connection_id.lock().await;
        let new_connection_id = ConnectionID(connection_id.0 + 1);
        *connection_id = new_connection_id.clone();
        drop(connection_id);

        let mut worker_connection_details = self.worker_connection_details.lock().await;
        worker_connection_details
            .connection_status
            .insert(new_connection_id.clone(), ConnectionStatus::Healthy);
        drop(worker_connection_details);

        let (mut write, mut read) = ws_stream.split();
        while let Some(message) = read.next().await {
            match message {
                Ok(message) => {
                    let message = String::from_utf8(message.into_data().to_vec()).unwrap();

                    let connection_message: ConnectionMessage = match serde_json::from_str(&message)
                    {
                        Ok(message) => message,
                        Err(_) => continue,
                    };

                    match connection_message.message_type {
                        ConnectionMessageTypes::NewJob => {
                            if !self.check_connection_status(&new_connection_id).await {
                                return;
                            }
                            let mut job = job_mutex.lock().await;

                            let message = ConnectionMessage::default_message_with_type(
                                ConnectionMessageTypes::MessageNotReady,
                            );
                            let mut message = serde_json::to_string(&message).unwrap();

                            if let Some(proof_job) = job.clone() {
                                let id = proof_job.identifier.clone();
                                let proof_job_str = serde_json::to_string(&proof_job).unwrap();
                                let job_msg = ConnectionMessage {
                                    message_type: ConnectionMessageTypes::NewJob,
                                    message: MessageData {
                                        transform_attempt_id: id,
                                        data: proof_job_str,
                                    },
                                };

                                message = serde_json::to_string(&job_msg).unwrap();
                                self.worker_connection_details
                                    .lock()
                                    .await
                                    .assigned_jobs
                                    .insert(
                                        (proof_job.identifier.clone(), new_connection_id.clone()),
                                        JobDetails {
                                            transform_attempt: proof_job.clone(),
                                            assigned_at: Instant::now(),
                                        },
                                    );
                                *job = None;
                                drop(job);
                                log::info!(
                                    "making new job of request id: {:?}",
                                    proof_job.identifier.transform_request_id
                                );
                            }
                            write.send(message.into()).await.unwrap();
                            write.flush().await.unwrap();
                        }
                        ConnectionMessageTypes::JobResult => {
                            log::info!(
                                "received job result from prover with connection id: {new_connection_id:?}",
                            );
                            if !self.check_connection_status(&new_connection_id).await {
                                return;
                            }

                            let attempt_id = connection_message.message.transform_attempt_id;
                            if self
                                .worker_connection_details
                                .lock()
                                .await
                                .assigned_jobs
                                .remove(&(attempt_id.clone(), new_connection_id.clone()))
                                .is_some()
                            {
                                let worker_manager_result: WorkerManagerResult<
                                    TwineBatchTransformAttempt,
                                > = serde_json::from_str(&connection_message.message.data).unwrap(); // TODO: write some message
                                log::info!(
                                    "received job result for job with request id: {:?}",
                                    attempt_id.transform_request_id
                                );
                                _sender.send(worker_manager_result).await.unwrap();
                            } else {
                                log::error!(
                                    "ignored job result received from different connection boundry"
                                );
                            }
                        }
                        ConnectionMessageTypes::KeepAlive => {
                            let message = ConnectionMessage::default_message_with_type(
                                ConnectionMessageTypes::KeepAlive,
                            );
                            let message = serde_json::to_string(&message).unwrap();

                            write.send(message.into()).await.unwrap();
                            write.flush().await.unwrap();
                            log::info!("keep alive signal from prover {new_connection_id:?}");
                        }
                        ConnectionMessageTypes::InvalidParams
                        | ConnectionMessageTypes::MessageNotReady => {} /* todo: terminate
                                                                         * connection */
                    }
                }
                Err(e) => log::error!("{e}"),
            }
            self.handle_assigned_jobs(_sender.clone()).await;
        }
    }

    async fn check_connection_status(&self, connection_id: &ConnectionID) -> bool {
        if let Some(connection_status) = self
            .worker_connection_details
            .lock()
            .await
            .connection_status
            .get(connection_id)
        {
            if connection_status.clone() == ConnectionStatus::Disconnected {
                log::warn!("connection status: Disconnected");
                return false;
            }
            return true;
        }
        false
    }

    /// manages job assignments timeouts
    pub async fn handle_assigned_jobs(
        &self,
        sender: Sender<WorkerManagerResult<TwineBatchTransformAttempt>>,
    ) {
        let last_timed_out_check = {
            let last_timed_out_check = self.last_timed_out_check.lock().await;
            *last_timed_out_check
        };
        if last_timed_out_check.elapsed().as_secs() > self.job_completion_timeout {
            log::info!("Checking for timed out jobs");

            let jobs: Vec<((TwineBatchTransformAttemptID, ConnectionID), JobDetails)> = {
                let worker_connection_details = self.worker_connection_details.lock().await;
                log::debug!(
                    "total assigned jobs {}",
                    worker_connection_details.assigned_jobs.len()
                );
                worker_connection_details
                    .assigned_jobs
                    .iter()
                    .filter(|(_, job_details)| {
                        job_details.assigned_at.elapsed().as_secs() > self.job_completion_timeout
                    })
                    .map(|(job_id, job_details)| (job_id.clone(), job_details.clone()))
                    .collect()
            };

            {
                let mut last_timed_out_check = self.last_timed_out_check.lock().await;
                *last_timed_out_check = Instant::now();
            }

            if jobs.is_empty() {
                log::info!("no timed out jobs found");
                return;
            }

            log::warn!(
                "found some timed out jobs. total timed out jobs {}",
                jobs.len()
            );
            for (job_id, job_details) in jobs {
                {
                    let mut worker_connection_details = self.worker_connection_details.lock().await;
                    worker_connection_details.assigned_jobs.remove(&job_id);
                    log::warn!("removed from assigned jobs {job_id:?}");
                    worker_connection_details
                        .connection_status
                        .remove(&job_id.1);
                    log::warn!("removed from connection {:?}", job_id.1);
                }
                let return_package = (
                    job_id.0.clone(),
                    TwineBatchTransformReturnCtx {
                        call_context: job_details.transform_attempt.call_ctx,
                        call_type: job_details.transform_attempt.call_val,
                        extra_data: vec![],
                    },
                    Err(ProofSchedulerError::Other("job timed out".to_string())),
                );

                let result: WorkerManagerResult<TwineBatchTransformAttempt> =
                    WorkerManagerResult::Failure(job_id.0, return_package);
                sender.send(result).await.unwrap();
            }
        }
    }
}
