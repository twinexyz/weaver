//! Ethereum writer service

use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use alloy_provider::{DynProvider, Provider};
use eyre::{eyre, WrapErr};
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::task::JoinSet;
use tracing::{debug, error, info, warn};

use crate::fees::FeeConfig;
use crate::metrics::TransactionServiceMetrics;
use crate::monitor::TransactionMonitoringHandle;
use crate::signer::{Signer, SignerEvent, SignerTask};
use crate::store::{StorageError, TransactionStorage};
use crate::transaction::{EthereumTransaction, PendingTransaction, TransactionStatus, TxId};

const STATUS_CHANNEL_CAPACITY: usize = 2048;
const CHANNEL_CAPACITY: usize = 1000;

/// Messages accepted by the [`TransactionService`].
#[derive(Debug)]
pub enum TransactionServiceMessage {
    /// Message to send a transaction and receive events about the status of the
    /// transaction.
    SendTransaction(
        Box<EthereumTransaction>,
        broadcast::Sender<TransactionStatus>,
        oneshot::Sender<eyre::Result<()>>,
    ),
    /// Subscribe to status updates for a specific transaction.
    Subscribe(
        TxId,
        oneshot::Sender<Option<broadcast::Receiver<TransactionStatus>>>,
    ),
}

/// Handle to communicate with the [`TransactionService`].
#[derive(Debug, Clone)]
pub struct TransactionServiceHandle {
    storage: TransactionStorage,
    command_tx: mpsc::Sender<TransactionServiceMessage>,
}

impl TransactionServiceHandle {
    /// Returns a clone of the underlying storage handle.
    pub fn storage(&self) -> TransactionStorage { self.storage.clone() }

    /// Sends a transaction to the service and returns a receiver for status
    /// updates.
    pub async fn submit_transaction(
        &self,
        tx: EthereumTransaction,
    ) -> eyre::Result<broadcast::Receiver<TransactionStatus>> {
        let (status_tx, status_rx) = broadcast::channel(STATUS_CHANNEL_CAPACITY);
        let (resp_tx, resp_rx) = oneshot::channel();

        self.command_tx
            .send(TransactionServiceMessage::SendTransaction(
                Box::new(tx),
                status_tx,
                resp_tx,
            ))
            .await
            .map_err(|err| eyre!("transaction service command channel closed: {err}"))?;

        match resp_rx
            .await
            .map_err(|err| eyre!("transaction service dropped submission response: {err}"))?
        {
            Ok(()) => Ok(status_rx),
            Err(err) => Err(err),
        }
    }

    /// Subscribes to updates for the provided transaction identifier.
    pub async fn subscribe(
        &self,
        tx_id: TxId,
    ) -> eyre::Result<Option<broadcast::Receiver<TransactionStatus>>> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.command_tx
            .send(TransactionServiceMessage::Subscribe(tx_id, resp_tx))
            .await
            .map_err(|err| eyre!("transaction service command channel closed: {err}"))?;

        resp_rx
            .await
            .map_err(|err| eyre!("transaction service dropped subscription response: {err}"))
    }
}

/// Service that handles transactions by dispatching outgoing transaction to the
/// signer and monitors the state of the transaction.
/// Receives incoming [`EthereumTransaction`] requests and sends those
/// transactions
#[derive(derive_more::Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct TransactionService {
    /// Task for signers to execute transactions
    signer: SignerTask,
    /// Message channel from signers to this service.
    from_signers: mpsc::Receiver<SignerEvent>,
    /// Incoming messages for the service.
    command_rx: mpsc::Receiver<TransactionServiceMessage>,
    /// Subscriptions to transaction status updates back to the initiator of the
    /// transaction.
    subscriptions: HashMap<TxId, broadcast::Sender<TransactionStatus>>,
    /// Metrics of the service.
    metrics: Arc<TransactionServiceMetrics>,
    /// Queue of transactions waiting for signers capacity.
    queue: TxQueue,
    /// Storage of the relay.
    storage: TransactionStorage,
    /// Set of spawned tasks that are terminated when the service is dropped.
    tasks: JoinSet<Result<(), StorageError>>,
}

impl TransactionService {
    /// Creates a new [`TransactionService`].
    ///
    /// This also spawns dedicated [`Signer`] task for each configured signer.
    pub async fn new(
        provider: DynProvider,
        private_key: &str,
        storage: TransactionStorage,
        config: TransactionServiceConfig,
        fees: FeeConfig,
    ) -> eyre::Result<(Self, TransactionServiceHandle)> {
        let chain_id = provider.get_chain_id().await?;
        let metrics = Arc::new(TransactionServiceMetrics::new(chain_id));
        let cloned_metrics = Arc::clone(&metrics);
        let cloned_metrics2 = Arc::clone(&metrics);

        let (command_tx, command_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (to_service, from_signers) = mpsc::channel(CHANNEL_CAPACITY);

        let monitor = TransactionMonitoringHandle::new(provider.clone());

        let (signer_task, pending_transactions) = Self::create_signer(
            provider.clone(),
            private_key,
            storage.clone(),
            to_service.clone(),
            config.clone(),
            cloned_metrics,
            monitor,
            fees.clone(),
        )
        .await?;

        let mut this = Self {
            signer: signer_task,
            from_signers,
            command_rx,
            subscriptions: Default::default(),
            metrics: cloned_metrics2,
            queue: TxQueue::with_capacity(config.max_queued_transactions),
            storage: storage.clone(),
            tasks: JoinSet::new(),
        };

        for pending in pending_transactions {
            let tx_id = pending.id();
            let (status_tx, _) = broadcast::channel(STATUS_CHANNEL_CAPACITY);
            this.subscriptions.insert(tx_id, status_tx);
            this.signer.resume_pending(pending);
            this.metrics.increment_pending();
        }

        let queued_txns = storage
            .read_queued_transactions(chain_id)
            .await
            .wrap_err("failed to load queued transactions from storage")?;

        if queued_txns.len() > config.max_queued_transactions {
            tracing::error!("increase max queued transactions to proceed");
            eyre::bail!("Current length: {} \nMax supported: {}\nChange the config `config.max_queued_transactions`", queued_txns.len(), config.max_queued_transactions);
        }

        for tx in queued_txns {
            let (status_tx, _) = broadcast::channel(STATUS_CHANNEL_CAPACITY);
            this.send_transaction(tx, status_tx)
                .wrap_err("failed to restore queued transaction from storage")?;
        }

        let handle = TransactionServiceHandle {
            command_tx,
            storage,
        };

        Ok((this, handle))
    }

    async fn create_signer(
        provider: DynProvider,
        private_key: &str,
        storage: TransactionStorage,
        to_service: mpsc::Sender<SignerEvent>,
        config: TransactionServiceConfig,
        metrics: Arc<TransactionServiceMetrics>,
        monitor: TransactionMonitoringHandle,
        fees: FeeConfig,
    ) -> eyre::Result<(SignerTask, Vec<PendingTransaction>)> {
        let signer = Signer::new(
            provider,
            private_key,
            storage,
            to_service,
            metrics,
            config,
            monitor,
            fees,
        )
        .await?;
        signer.into_future().await
    }

    fn send_transaction(
        &mut self,
        tx: EthereumTransaction,
        status_tx: broadcast::Sender<TransactionStatus>,
    ) -> eyre::Result<()> {
        debug_assert!(
            !self.subscriptions.contains_key(&tx.id),
            "tx subscription already exists {}",
            tx.id
        );

        let tx_id = tx.id;
        self.subscriptions.insert(tx_id, status_tx);
        if let Err(err) = self.push_to_queue(tx) {
            self.subscriptions.remove(&tx_id);
            return Err(err);
        }

        Ok(())
    }

    /// Pushes a transaction to the queue.
    fn push_to_queue(&mut self, tx: EthereumTransaction) -> eyre::Result<()> {
        let tx_id = tx.id;
        let tx_hash = tx_id.into();
        let storage = self.storage.clone();
        let existing = tokio::task::block_in_place(move || {
            Handle::current()
                .block_on(storage.read_transaction_status(tx_hash))
                .ok()
                .flatten()
        });

        if let Some((_, status)) = existing {
            tracing::info!(?status, "transaction already in storage. skipping..");
            return Ok(());
        }

        match self.queue.push_transaction(tx) {
            Ok(queued) => {
                if queued {
                    self.metrics.increment_queued();
                }
                Ok(())
            }
            Err(err) => {
                self.metrics.record_queue_rejection();
                Err(err)
            }
        }
    }
}

/// Configuration for transaction service.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransactionServiceConfig {
    /// Maximum number of transactions allowed to sit in the in-memory queue.
    pub max_queued_transactions: usize,
    /// Interval for checking signer balances.
    pub balance_check_interval: Duration,
    /// Interval for checking nonce gaps.
    pub nonce_check_interval: Duration,
    /// Timeout after which we consider transaction as failed, in seconds.
    pub transaction_timeout: Duration,
    /// Whether to skip transaction simulation before broadcasting.
    pub skip_transaction_simulation: bool,
}

impl Default for TransactionServiceConfig {
    fn default() -> Self {
        Self {
            max_queued_transactions: CHANNEL_CAPACITY,
            balance_check_interval: Duration::from_secs(60 * 10), // every 10 minutes
            nonce_check_interval: Duration::from_secs(60),
            transaction_timeout: Duration::from_secs(60 * 5), // every 5 minutes
            skip_transaction_simulation: false,
        }
    }
}

impl Future for TransactionService {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        // Drain and log signer messages
        while let Poll::Ready(Some(event)) = this.from_signers.poll_recv(cx) {
            match event {
                SignerEvent::TransactionStatus(id, status) => {
                    match status.clone() {
                        TransactionStatus::Confirmed(receipt) => {
                            info!(tx_id = %id, %receipt.transaction_hash, "transaction confirmed");
                            this.metrics.record_confirmed();
                        }
                        TransactionStatus::Failed(err) => {
                            warn!(tx_id = %id, ?err, "transaction failed");
                            this.metrics.record_failed();
                        }
                        _ => {}
                    }
                    if let Some(status_tx) = this.subscriptions.get(&id) {
                        let _ = status_tx.send(status.clone());
                    }

                    if status.is_final() {
                        this.subscriptions.remove(&id);
                    }
                }
            }
        }

        // Drain commands
        while let Poll::Ready(action_opt) = this.command_rx.poll_recv(cx) {
            if let Some(action) = action_opt {
                match action {
                    TransactionServiceMessage::SendTransaction(tx, status_tx, resp_tx) => {
                        let result = this.send_transaction(*tx, status_tx);
                        if let Err(err) = &result {
                            error!(?err, "failed to enqueue transaction");
                        }
                        let _ = resp_tx.send(result);
                    }
                    TransactionServiceMessage::Subscribe(tx_id, status_tx) => {
                        let _ =
                            status_tx.send(this.subscriptions.get(&tx_id).map(|s| s.subscribe()));
                    }
                }
            } else {
                // command channel closed, shut down
                debug!("command channel closed");
                return Poll::Ready(());
            }
        }

        // Try advancing the queue.
        while this.queue.has_ready() {
            if let Some(tx) = this.queue.pop_ready() {
                this.signer.push_transaction(tx);
                this.metrics.record_sent();
                this.metrics.decrement_queued();
            }
        }

        while let Poll::Ready(Some(result)) = this.tasks.poll_join_next(cx) {
            if !matches!(result, Ok(Ok(_))) {
                error!("tx service task failed: {:?}", result);
            }
        }

        Poll::Pending
    }
}

/// Pool of transactions managed by the service.
///
/// Transaction lifecycle:
/// - [`TxQueue::push_transaction`] must be called for every queued transaction.
/// - [`TxQueue::pop_ready`] yields a transaction that is ready to be sent
/// - [`TxQueue::push_front`] queue nat first
#[derive(Debug)]
struct TxQueue {
    /// Queue of transactions that are ready to be sent.
    ready: VecDeque<EthereumTransaction>,
    /// Maximum number of transactions allowed in the queue at any time.
    capacity: usize,
}

impl TxQueue {
    /// Creates a new [`TxQueue`].
    fn with_capacity(capacity: usize) -> Self {
        Self {
            ready: VecDeque::new(),
            capacity,
        }
    }

    /// Returns whether there are any transactions ready to be sent.
    fn has_ready(&self) -> bool { !self.ready.is_empty() }

    /// Returns the configured capacity of the queue.
    fn capacity(&self) -> usize { self.capacity }

    /// Returns the number of queued transactions.
    fn len(&self) -> usize { self.ready.len() }

    /// Returns whether the queue is at capacity.
    fn is_full(&self) -> bool { self.len() >= self.capacity }

    /// Pushes a transaction to the queue.
    fn push_transaction(&mut self, tx: EthereumTransaction) -> eyre::Result<bool> {
        if self.is_full() {
            eyre::bail!(
                "transaction queue is full (limit {}, queued {})",
                self.capacity(),
                self.len()
            );
        }
        self.ready.push_back(tx);
        Ok(true)
    }

    /// Returns the next transaction from the ready queue
    fn pop_ready(&mut self) -> Option<EthereumTransaction> { self.ready.pop_front() }

    /// Retry transaction in case of failure,
    /// Use in case highest priority txn is required
    #[allow(dead_code)]
    fn push_front(&mut self, tx: EthereumTransaction) { self.ready.push_front(tx); }
}
