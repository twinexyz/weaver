//! client to connect to the worker manager

use std::sync::Arc;
use std::time::Duration;

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;
use tokio::time::{interval, sleep};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use twine_l2_proof_scheduler::batch_transform::transform_attempt::{
    TwineBatchTransformAttempt, TwineBatchTransformAttemptID,
};
use twine_l2_proof_scheduler::batch_transform::transform_request::TwineBatchTransformRequestID;
use twine_l2_proof_scheduler::worker_manager::connections::{
    ConnectionMessage, ConnectionMessageTypes, MessageData,
};

use crate::errors::ProverError;

/// ws stream writer
pub type StreamWriter = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;
/// ws stream reader
pub type StreamReader = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

/// Client to connect to the worker manager
#[derive(Debug)]
pub struct WSSClient {
    /// worker manager url
    pub worker_manager_url: String,
    /// Job type
    pub worker_to_manager_message_receiver: Receiver<ConnectionMessage>,
    /// message sender
    pub job_from_wss_sender: Sender<TwineBatchTransformAttempt>,
    /// keep alive request interval
    pub keep_alive_signal_interval: u64,
}

impl WSSClient {
    /// Creates new instance of WSSClient
    pub fn new(
        worker_manager_url: String,
        worker_to_manager_message_receiver: Receiver<ConnectionMessage>,
        job_from_wss_sender: Sender<TwineBatchTransformAttempt>,
        keep_alive_signal_interval: u64,
    ) -> Self {
        Self {
            worker_manager_url,
            worker_to_manager_message_receiver,
            job_from_wss_sender,
            keep_alive_signal_interval,
        }
    }

    /// Handles connection to the worker manager server
    pub async fn connection_loop(&mut self) {
        let ws_stream;
        loop {
            if let Ok((stream, _)) = connect_async(&self.worker_manager_url).await {
                ws_stream = stream;
                log::info!("Connected to worker manager's message stream");
                break;
            }
            sleep(Duration::from_secs(10)).await;
            log::warn!("Could not connect to ws stream. retrying...")
        }
        let (write, mut read) = ws_stream.split();
        log::info!("WebSocket handshake has been successfully completed");
        let write = Arc::new(Mutex::new(write));

        let mut keep_alive_interval =
            interval(Duration::from_secs(self.keep_alive_signal_interval));

        loop {
            tokio::select! {
                _ = keep_alive_interval.tick() => {
                    let keep_alive_request = ConnectionMessage::default_message_with_type(ConnectionMessageTypes::KeepAlive);
                    self.ws_message_writer(&keep_alive_request, write.clone()).await
                }
                Some(connection_message) = self.worker_to_manager_message_receiver.recv() => {
                    self.ws_message_writer(&connection_message, write.clone()).await
                }
                Some(message) = read.next() => {
                    let message = match message {
                        Ok(message) => message,
                        Err(e) => {
                            log::error!("error message received, {e}");
                            continue;
                        }
                    };
                    match self.send_ws_message_to_processor(message).await {
                        Ok(_) => {},
                        Err(e) => {

                            match e {
                                ProverError::MessageNotReady => {
                                    let error_message = ConnectionMessage::default_message_with_type(ConnectionMessageTypes::NewJob);
                                    sleep(Duration::from_secs(self.keep_alive_signal_interval)).await;
                                    self.ws_message_writer(&error_message, write.clone()).await;
                                    log::info!("message not ready, resending new job request");
                               }
                                _ => {
                                    let error_message =
                                        ConnectionMessage {
                                            message_type: ConnectionMessageTypes::InvalidParams,
                                            message: MessageData {
                                                        transform_attempt_id: TwineBatchTransformAttemptID::new(
                                                        0,
                                                        TwineBatchTransformRequestID { identifier: 0 },
                                                    ),
                                                        data: format!("{e}"),
                                                    },
                                        };
                                    sleep(Duration::from_secs(self.keep_alive_signal_interval)).await;
                                    self.ws_message_writer(&error_message, write.clone()).await;
                                    log::error!("error from prover, sending error message to worker manager: {e}")
                               }
                            }

                        }
                    }
                }
            }
        }
    }

    async fn ws_message_writer(
        &self,
        connection_message: &ConnectionMessage,
        writer: Arc<Mutex<StreamWriter>>,
    ) {
        let message = serde_json::to_string(connection_message).unwrap();
        let message_buf = message.as_bytes().to_vec();

        writer.lock().await.send(message_buf.into()).await.unwrap();
    }

    /// receives the message from the ws stream and sends the message to the
    /// processors
    pub async fn send_ws_message_to_processor(&self, message: Message) -> Result<(), ProverError> {
        let message = String::from_utf8(message.into_data().to_vec())
            .map_err(|e| ProverError::Other(e.to_string()))?;
        let connection_message: ConnectionMessage =
            serde_json::from_str(&message).map_err(|e| ProverError::Other(e.to_string()))?;
        match connection_message.message_type {
            ConnectionMessageTypes::NewJob => {
                log::debug!("new job from the scheduler");
                let transform_attempt: TwineBatchTransformAttempt =
                    serde_json::from_str(&connection_message.message.data)
                        .map_err(|e| ProverError::Other(e.to_string()))?;
                self.job_from_wss_sender
                    .send(transform_attempt)
                    .await
                    .map_err(|e| ProverError::Other(e.to_string()))
            }
            ConnectionMessageTypes::MessageNotReady => Err(ProverError::MessageNotReady),
            ConnectionMessageTypes::KeepAlive => {
                log::info!("keep alive signal from the server");
                Ok(())
            }
            _ => Err(ProverError::UnexpectedMessageType),
        }
    }
}
