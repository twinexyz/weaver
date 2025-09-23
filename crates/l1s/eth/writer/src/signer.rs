//! Signer Details

use std::sync::Arc;
use std::time::Duration;

use alloy_consensus::{Transaction, TxEip1559, TxEnvelope, TypedTransaction};
use alloy_eips::eip1559::Eip1559Estimation;
use alloy_eips::{BlockId, Encodable2718};
use alloy_primitives::hex::FromHex;
use alloy_primitives::{Address, B256};
use alloy_provider::network::{Ethereum, EthereumWallet, NetworkWallet};
use alloy_provider::{DynProvider, Provider};
use alloy_rpc_types::{TransactionReceipt, TransactionRequest};
use alloy_signer_local::PrivateKeySigner;
use alloy_transport::{RpcError, TransportErrorKind};
use chrono::Utc;
use eyre::{Context, OptionExt};
use futures_util::lock::Mutex;
use futures_util::stream::FuturesUnordered;
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio::time::Instant;
use tracing::{debug, error, info, instrument, span, trace, warn, Instrument, Level};

use crate::cast_debug::generate_cast_call_command;
use crate::fees::{FeeConfig, FeeContext, FeesError, MIN_GAS_PRICE_BUMP};
use crate::metrics::{SignerMetrics, TransactionServiceMetrics};
use crate::monitor::TransactionMonitoringHandle;
use crate::service::TransactionServiceConfig;
use crate::store::{StorageError, TransactionStorage};
use crate::transaction::{
    EthereumTransaction, PendingTransaction, TransactionFailureReason, TransactionStatus, TxId,
};

/// The number of blocks from the past for which the fee rewards are fetched for
/// fee estimation.
pub const EIP1559_FEE_ESTIMATION_PAST_BLOCKS: u64 = 10;

/// Errors that may occur while sending a transaction.
#[derive(Debug, thiserror::Error)]
pub enum SignerError {
    /// The transaction was dropped.
    #[error("transaction was dropped")]
    TxDropped,

    /// The transaction timed out while waiting for confirmation.
    #[error("timed out while waiting for confirmation")]
    TxTimeout,

    /// The growth of the gas fees exceeded the amount we are ready to pay
    #[error("transaction underpriced: {0}")]
    FeesTooHigh(#[from] FeesError),

    /// Error occurred while signing transaction.
    #[error(transparent)]
    Sign(#[from] alloy_signer_local::LocalSignerError),

    /// RPC error.
    #[error(transparent)]
    Rpc(#[from] RpcError<TransportErrorKind>),

    /// Storage error.
    #[error(transparent)]
    Storage(#[from] StorageError),

    /// Other errors.
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
}

/// Event emitted by the [`Signer`].
#[derive(Debug)]
pub enum SignerEvent {
    /// Status update for a transaction.
    TransactionStatus(TxId, TransactionStatus),
}

/// A signer responsible for signing and sending transactions on a _single_
/// network.
#[derive(Debug, Clone, derive_more::Deref)]
pub struct Signer {
    #[deref]
    inner: Arc<SignerInner>,
}

#[derive(Debug)]
/// Signer State
pub struct SignerInner {
    /// Provider
    provider: Arc<DynProvider>,
    /// Chain Id
    chain_id: u64,
    /// Estimated block time
    block_time: Duration,
    /// wallet to sign transactions
    wallet: EthereumWallet,
    /// nonce of this signer
    nonce: Mutex<u64>,
    /// Channel to send signer events to.
    events_tx: mpsc::Sender<SignerEvent>,
    /// Underlying storage.
    storage: TransactionStorage,
    /// Metrics of the parent transaction service.
    metrics: SignerMetrics,
    /// Configuration for the service.
    config: TransactionServiceConfig,
    /// Handle for monitoring pending transactions.
    monitor: TransactionMonitoringHandle,
    /// Fee settings for the network this signer supports.
    fees: FeeConfig,
}

impl Signer {
    #[expect(clippy::too_many_arguments)]
    /// Create a new signer
    pub async fn new(
        provider: DynProvider,
        private_key: &str,
        storage: TransactionStorage,
        events_tx: mpsc::Sender<SignerEvent>,
        tx_metrics: Arc<TransactionServiceMetrics>,
        config: TransactionServiceConfig,
        monitor: TransactionMonitoringHandle,
        fees: FeeConfig,
    ) -> eyre::Result<Self> {
        let signer = PrivateKeySigner::from_bytes(
            &B256::from_hex(private_key).context("Invalid private key hex")?,
        )
        .context("Failed building signer")?;
        let address = signer.address();

        let wallet = EthereumWallet::new(signer);

        // fetch account info
        let (nonce, chain_id, latest) = tokio::try_join!(
            provider.get_transaction_count(address).pending(),
            provider.get_chain_id(),
            provider.get_block(BlockId::latest())
        )?;

        // Heuristically estimate the block time.
        let estimated_block_time = {
            let latest = latest.ok_or_eyre("couldn't fetch latest block")?;
            let latest_number = latest.header.number;

            if latest_number <= 1 {
                Duration::from_secs(1)
            } else {
                let length = 1000.min(latest_number - 1);
                let start_number = latest_number - length;
                let start = provider
                    .get_block(BlockId::number(start_number))
                    .await?
                    .ok_or_eyre("couldn't fetch block to estimate block time")?;

                let elapsed = latest
                    .header
                    .timestamp
                    .saturating_sub(start.header.timestamp);
                let per_block = elapsed / length.max(1);
                if per_block == 0 {
                    Duration::from_secs(1)
                } else {
                    Duration::from_millis(1000 * per_block)
                }
            }
        };

        let inner = SignerInner {
            provider: provider.into(),
            chain_id,
            block_time: estimated_block_time,
            wallet,
            nonce: nonce.into(),
            storage,
            events_tx,
            metrics: SignerMetrics::new(tx_metrics, chain_id, address),
            config,
            monitor,
            fees,
        };

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Returns the signer address.
    pub fn address(&self) -> Address {
        NetworkWallet::<Ethereum>::default_signer_address(&self.wallet)
    }

    /// Returns the chain id.
    pub fn chain_id(&self) -> u64 { self.chain_id }

    /// Emits an event.
    fn emit_event(&self, event: SignerEvent) {
        if let Err(err) = self.events_tx.try_send(event) {
            debug!(?err, "failed to emit signer event");
        }
    }

    /// Sends a transaction status update.
    async fn update_tx_status(
        &self,
        tx: TxId,
        status: TransactionStatus,
    ) -> Result<(), StorageError> {
        self.storage.write_transaction_status(tx, &status).await?;

        match &status {
            TransactionStatus::Pending(_) => self.metrics.tx_metrics.increment_pending(),
            status if status.is_final() => self.metrics.tx_metrics.decrement_pending(),
            _ => {}
        }

        self.emit_event(SignerEvent::TransactionStatus(tx, status));

        Ok(())
    }

    /// Estimates the [`Eip1559Estimation`] with the configured settings.
    async fn estimate_eip1559_fees(&self) -> eyre::Result<Eip1559Estimation> {
        let fees = self.provider.estimate_eip1559_fees().await?;
        let adjusted_fees = self.fees.adjusted_eip1559_estimation(fees);
        Ok(adjusted_fees)
    }

    /// State and metrics update on successful transaction
    async fn on_confirmed_transaction(
        &self,
        tx: PendingTransaction,
        receipt: TransactionReceipt,
    ) -> Result<(), StorageError> {
        self.update_tx_status(
            tx.id(),
            TransactionStatus::Confirmed(Box::new(receipt.clone())),
        )
        .await?;
        self.storage.remove_pending_transaction(tx.id()).await?;
        self.metrics.tx_metrics.decrement_pending();

        Ok(())
    }

    /// Invoked when a transaction fails.
    async fn on_failed_transaction(
        &self,
        tx: TxId,
        err: impl TransactionFailureReason + 'static,
    ) -> Result<(), SignerError> {
        // Remove transaction from storage
        self.storage.remove_queued(tx).await?;
        self.storage.remove_pending_transaction(tx).await?;

        // Update status
        self.update_tx_status(tx, TransactionStatus::failed(err))
            .await?;
        self.metrics.tx_metrics.decrement_pending();

        Ok(())
    }

    /// Fetches the [`FeeContext`].
    async fn get_fee_context(&self) -> Result<FeeContext, SignerError> {
        let fee_history = self
            .provider
            .get_fee_history(EIP1559_FEE_ESTIMATION_PAST_BLOCKS, Default::default(), &[
                self.fees.priority_fee_percentile,
            ])
            .await?;

        let last_base_fee = fee_history.latest_block_base_fee().unwrap_or_default();
        let fee_estimate = self.fees.estimate_eip1559_fees(&fee_history);

        Ok(FeeContext {
            last_base_fee,
            recommended_priority_fee: fee_estimate.max_priority_fee_per_gas,
        })
    }

    #[instrument(skip_all, fields(chain_id = %self.chain_id))]
    async fn validate_transaction(
        &self,
        tx: &mut EthereumTransaction,
        fees: Eip1559Estimation,
    ) -> Result<(), SignerError> {
        // Individual transaction config
        if tx.skip_simulation {
            debug!(tx_id = %tx.id, "skipping transaction simulation as per configuration");
            return Ok(());
        }

        // Skip simulation for all transactions
        if self.config.skip_transaction_simulation {
            debug!(tx_id = %tx.id, "skipping transaction simulation as per configuration");
            return Ok(());
        }

        let mut request: TransactionRequest = tx.build(0, fees).into();
        request.nonce = None;
        request.from = Some(self.address());
        let mut attempts = 0;
        loop {
            let result = self
                .provider
                .call(request.clone())
                .await
                .map_err(SignerError::from);
            if result.is_ok() {
                break;
            } else if attempts < 4 {
                attempts += 1;
                debug!(error = ?result, ?request, chain_id = self.chain_id, "transaction simulation failed retrying... (attempt {}/5)", attempts);
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            } else {
                error!(?result, ?request, cast_call = %generate_cast_call_command(&request), "transaction simulation failed");
                result?;
            }
        }

        Ok(())
    }

    /// Signs a given transaction.
    #[instrument(skip_all)]
    async fn sign_transaction(&self, tx: TypedTransaction) -> Result<TxEnvelope, SignerError> {
        Ok(
            NetworkWallet::<Ethereum>::sign_transaction_from(&self.wallet, self.address(), tx)
                .await
                .map_err(|e| {
                    error!(error=?e, "sign transaction error");
                    SignerError::Other(e.into())
                })?,
        )
    }

    /// Broadcasts a given transaction.
    #[instrument(skip_all, fields(chain_id = %self.chain_id))]
    async fn send_transaction(&self, tx: &TxEnvelope) -> Result<(), SignerError> {
        let _ = self
            .provider
            .send_raw_transaction(&tx.encoded_2718())
            .await
            .inspect(|_| {
                trace!(
                    tx_hash = %tx.hash(),
                    nonce = %tx.nonce(),
                    "sent transaction"
                );
            })
            .inspect_err(|err| {
                error!(
                    ?tx,
                    tx_hash = %tx.hash(),
                    nonce = %tx.nonce(),
                    err = %err,
                    "failed to send transaction"
                );
            })?;

        Ok(())
    }

    #[instrument(skip_all, fields(tx_id = %tx.tx.id))]
    async fn watch_transaction(&self, mut tx: PendingTransaction) -> Result<(), SignerError> {
        match self.watch_transaction_inner(&mut tx).await {
            Ok(receipt) => {
                self.on_confirmed_transaction(tx, receipt).await?;
            }
            Err(err) => {
                if matches!(err, SignerError::TxTimeout) {
                    self.metrics.tx_metrics.record_timeout();
                }
                error!(
                    ?err,
                    "failed to wait for transaction confirmation, closing nonce gap"
                );

                // If we've failed to send the transaction, start closing the nonce gap to make
                // sure we occupy the chosen nonce.
                self.close_nonce_gap(tx.nonce(), Some(tx.fees())).await;

                for sent in &tx.sent {
                    if let Ok(Some(receipt)) =
                        self.provider.get_transaction_receipt(*sent.tx_hash()).await
                    {
                        if receipt.block_number.is_some() {
                            self.on_confirmed_transaction(tx, receipt).await?;
                            return Ok(());
                        }
                    }
                }

                self.on_failed_transaction(tx.id(), err).await?;
            }
        }
        Ok(())
    }

    /// Waits for a pending transaction to be confirmed.
    ///
    /// Receives a mutable reference to [`SentTransaction`] and might
    /// potentially modify it when bumping the fees.
    #[instrument(skip_all, fields(chain_id = %self.chain_id))]
    async fn watch_transaction_inner(
        &self,
        tx: &mut PendingTransaction,
    ) -> Result<TransactionReceipt, SignerError> {
        let mut last_sent_at = Instant::now();
        loop {
            if last_sent_at.elapsed() >= self.config.transaction_timeout {
                error!(?tx, "Transaction timed out");
                return Err(SignerError::TxTimeout);
            }

            let mut handles = FuturesUnordered::new();
            for sent in &tx.sent {
                handles.push(
                    self.monitor
                        .watch_transaction(*sent.tx_hash(), self.block_time * 2),
                );
            }
            while let Some(receipt_opt) = handles.next().await {
                if let Some(receipt) = receipt_opt {
                    return Ok(receipt);
                }
            }

            let fees = self.get_fee_context().await?;
            let best_tx = tx.best_tx();

            if let Some(new_fees) =
                fees.prepare_replacement(best_tx, tx.tx.max_fee_for_transaction())?
            {
                let new_tx = tx.tx.build(tx.nonce(), new_fees);
                let replacement = self.sign_transaction(new_tx).await?;
                self.storage
                    .add_pending_envelope(tx.id(), &replacement)
                    .await?;
                self.send_transaction(&replacement).await?;
                self.update_tx_status(tx.id(), TransactionStatus::Pending(*replacement.tx_hash()))
                    .await?;
                tx.sent.push(replacement);
                last_sent_at = Instant::now();
                self.metrics.tx_metrics.record_replacement();
            } else {
                trace!("was not able to wait for tx confirmation, attempting to resend");
                if let Err(err) = self
                    .provider
                    .send_raw_transaction(&best_tx.encoded_2718())
                    .await
                {
                    let error_message = err.to_string();
                    let is_already_known = error_message.contains("already known");
                    let is_nonce_too_low = error_message.contains("nonce too low");
                    if !is_already_known && !is_nonce_too_low {
                        debug!(%err, "failed to resubmit transaction");
                    }
                }
            }
        }
    }

    /// Closes the nonce gap by sending a dummy transaction to the signer.
    ///
    /// This can be called in 2 cases:
    ///     1. We failed to send a transaction. This is very unlikely, and if
    ///        happens, hard to recover as it most likely signals critical KMS
    ///        or RPC failure.
    ///     2. We failed to wait for a transaction to be mined. This is more
    ///        likely, and means that transaction wa successfully broadcasted
    ///        but never confirmed likely causing a nonce gap.
    #[instrument(skip_all, fields(chain_id = %self.chain_id, %nonce))]
    async fn close_nonce_gap(&self, nonce: u64, min_fees: Option<Eip1559Estimation>) {
        let try_close = || async {
            let fee_estimate = self
                .estimate_eip1559_fees()
                .await
                .map_err(|e| (SignerError::Other(e.into()), B256::ZERO))?;

            let (max_fee, max_tip) = if let Some(min_fees) = min_fees {
                // If we are provided with `min_fees`, this means, we are going to replace some
                // existing transaction. Nodes usually require us to bump the fees by some
                // margin to replace a transaction, so we are enforcing that
                // assigned fees are not too low.
                let min_fee = min_fees.max_fee_per_gas * (100 + MIN_GAS_PRICE_BUMP) / 100;
                let min_tip = min_fees.max_priority_fee_per_gas * (100 + MIN_GAS_PRICE_BUMP) / 100;

                (
                    min_fee.max(fee_estimate.max_fee_per_gas),
                    min_tip.max(fee_estimate.max_priority_fee_per_gas),
                )
            } else {
                (
                    fee_estimate.max_fee_per_gas,
                    fee_estimate.max_priority_fee_per_gas,
                )
            };

            let tx = TypedTransaction::Eip1559(TxEip1559 {
                chain_id: self.chain_id,
                nonce,
                to: self.address().into(),
                gas_limit: 21000,
                max_priority_fee_per_gas: max_tip,
                max_fee_per_gas: max_fee,
                ..Default::default()
            });

            let tx = self
                .sign_transaction(tx)
                .await
                .map_err(|e| (e, B256::ZERO))?;

            let tx_hash = *tx.tx_hash();
            debug!(%tx_hash, "Sending nonce gap closing transaction");
            self.send_transaction(&tx).await.map_err(|e| (e, tx_hash))?;
            // Give transaction 10 blocks to be mined.
            if self
                .monitor
                .watch_transaction(tx_hash, self.block_time * 10)
                .await
                .is_none()
            {
                return Err((SignerError::TxTimeout, tx_hash));
            }

            Ok::<_, (SignerError, B256)>(())
        };

        loop {
            debug!("attempting to close nonce gap");

            let Err((err, tx_hash)) = try_close().await else {
                break;
            };

            error!(%tx_hash, %err, "failed to close nonce gap");

            if let Ok(latest_nonce) = self.provider.get_transaction_count(self.address()).await {
                if latest_nonce > nonce {
                    warn!("nonce gap was closed by a different transaction");
                    break;
                }

                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }

        debug!("closed nonce gap");
    }

    /// Broadcasts a given transaction and waits for it to be confirmed,
    /// notifying `status_tx` on each status update.
    async fn send_and_watch_transaction(
        &self,
        mut tx: EthereumTransaction,
    ) -> Result<(), SignerError> {
        // Fetch the fees for the first transaction.
        let fees = match self
            .get_fee_context()
            .await
            .and_then(|fees| Ok(fees.fees_for_new_transaction(tx.max_fee_for_transaction())?))
        {
            Ok(fees) => fees,
            Err(err) => {
                self.on_failed_transaction(tx.id, err).await?;
                return Ok(());
            }
        };

        // Validate the transaction.
        if let Err(err) = self.validate_transaction(&mut tx, fees).await {
            self.on_failed_transaction(tx.id, err).await?;
            return Ok(());
        }

        // Choose nonce for the transaction.
        let nonce = {
            let mut nonce = self.nonce.lock().await;
            let current_nonce = *nonce;
            *nonce += 1;
            current_nonce
        };

        let tx_id = tx.id;

        let try_send = async {
            // sign transaction
            let signed = self.sign_transaction(tx.build(nonce, fees)).await?;

            // write pending transaction to storage first to avoid race condition
            let tx = PendingTransaction {
                tx,
                sent: vec![signed.clone()],
                signer: self.address(),
                sent_at: Utc::now(),
            };
            self.storage.replace_queued_tx_with_pending(&tx).await?;
            self.metrics.tx_metrics.increment_pending();

            // send transaction and update status
            self.send_transaction(&signed).await?;
            self.update_tx_status(tx.id(), TransactionStatus::Pending(*signed.hash()))
                .await?;

            Ok::<_, SignerError>(tx)
        };

        match try_send.await {
            Ok(tx) => self.watch_transaction(tx).await,
            Err(err) => {
                error!(%err, tx_id = %tx_id, signer = %self.address(), chain_id = %self.chain_id, "failed to send a transaction");

                self.on_failed_transaction(tx_id, err).await?;

                // If no other transaction occupied the next nonce, we can just reset it.
                {
                    let mut lock = self.nonce.lock().await;
                    if *lock == nonce + 1 {
                        *lock = nonce;
                        return Ok(());
                    }
                }

                // Otherwise, we need to close the nonce gap.
                self.close_nonce_gap(nonce, None).await;

                Ok(())
            }
        }
    }

    /// Record balance
    pub async fn record_and_check_balance(&self) -> eyre::Result<()> {
        let signer = self.address();
        let balance = self.provider.get_balance(signer).await?;

        let balance_value = u64::try_from(balance).unwrap_or(u64::MAX);

        self.metrics.balance.set(balance_value as f64);

        Ok(())
    }

    /// Returns [`SignerTack`] and list of [`PendingTransactions`]
    /// the list of pending transactions is the txn list stored on storage layer
    pub async fn into_future(self) -> eyre::Result<(SignerTask, Vec<PendingTransaction>)> {
        let loaded_transactions = self
            .storage
            .read_pending_transactions(self.chain_id)
            .await
            .wrap_err("failed to read pending transactions")?;

        // Ensure pending transactions not getting overridden by new ones
        {
            let mut lock = self.nonce.lock().await;
            if let Some(nonce) = loaded_transactions.iter().map(|tx| tx.nonce() + 1).max() {
                if nonce > *lock {
                    *lock = nonce;
                }
            }
        }

        let pending = JoinSet::new();
        let mut maintenance = JoinSet::new();

        // Create a never ending task that checks if on-chain nonce has diverged from
        // local nonce
        let this = self.clone();
        maintenance.spawn(async move {
            loop {
                tokio::time::sleep(this.config.nonce_check_interval).await;

                if let Ok(nonce) =
                    this.provider.get_transaction_count(this.address()).pending().await
                {
                    this.metrics.nonce.absolute(nonce);
                    let mut lock = this.nonce.lock().await;
                    if nonce > *lock {
                        warn!(%nonce, signer = %this.address(), chain_id = %this.chain_id, "on-chain nonce is ahead of local");
                        *lock = nonce;
                    }
                }
            }
        });

        // create a never ending task that checks signer balance.
        let this = self.clone();
        maintenance.spawn(async move {
            loop {
                tokio::time::sleep(this.config.balance_check_interval).await;

                if let Err(err) = this.record_and_check_balance().await {
                    warn!(%err, signer = %this.address(), chain_id = %this.chain_id, "failed to check signer balance");
                }
            }
        });

        Ok((
            SignerTask {
                signer: self,
                pending,
                _maintenance: maintenance,
            },
            loaded_transactions,
        ))
    }
}

/// A never ending future operating on [`Signer`] and handling transactions
/// sending.
#[derive(derive_more::Debug)]
pub struct SignerTask {
    /// The signer instance.
    signer: Signer,
    /// All currently pending tasks. Those include pending transactions and
    /// nonce gap closing tasks.
    pending: JoinSet<Result<(), SignerError>>,
    /// Balance checking, nonce gap detection tasks
    _maintenance: JoinSet<()>,
}

impl SignerTask {
    /// Pushes a new traаnsaction to the signer.
    ///
    /// Note; the transaction sending future is not polled until the
    /// [`SignerTask`] is polled.
    pub fn push_transaction(&mut self, tx: EthereumTransaction) {
        let tx_id = tx.id;
        info!(?tx_id, " handling transaction ");
        let signer = self.signer.clone();
        self.pending.spawn(async move {
            let span = span!(
                Level::INFO,
                "process tx",
                messaging.message.id = %tx.id,
            );

            signer.send_and_watch_transaction(tx).instrument(span).await
        });
    }

    /// Resumes monitoring a previously sent transaction.
    pub fn resume_pending(&mut self, tx: PendingTransaction) {
        let signer = self.signer.clone();
        let tx_id = tx.id();
        self.pending.spawn(async move {
            let span = span!(
                Level::INFO,
                "resume tx",
                messaging.message.id = %tx_id,
            );

            signer.watch_transaction(tx).instrument(span).await
        });
    }

    /// Returns the number of pending transactions currently being processed by
    /// the signer.
    pub fn pending(&self) -> usize { self.pending.len() }
}
