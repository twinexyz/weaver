//! Twine solana provider and utilities

use std::time::Duration;

use eyre::Result;
use reth_tracing::tracing;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_sdk::commitment_config::{CommitmentConfig, CommitmentLevel};
use solana_sdk::instruction::Instruction;
use solana_sdk::message::Message;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{read_keypair_file, Signature};
use solana_sdk::signer::Signer;
use solana_sdk::transaction::Transaction;
use twine_l1::error::TransactionError;

pub mod query;

/// Solana Provider
#[derive(Debug, Clone)]
pub struct SolanaProvider {
    /// Solana rpc url
    pub rpc: String,
    /// Solana chain id
    pub chain_id: u64,
    /// Solana wallet to do the transactions
    pub admin_wallet_path: String,
    /// Pubkey of twine chain contract on solana
    pub twine_chain_program: Pubkey,
}

impl SolanaProvider {
    /// Initialize solana provider
    pub fn new(
        rpc: String,
        chain_id: u64,
        twine_chain_program: &str,
        admin_wallet_path: String,
    ) -> Self {
        let twine_chain = Pubkey::from_str_const(twine_chain_program);
        Self {
            rpc,
            chain_id,
            admin_wallet_path,
            twine_chain_program: twine_chain,
        }
    }
}

impl SolanaProvider {
    /// Waits for the tx to reach the desired commitment level and returns
    ///
    /// Ok()        – transaction succeeded
    ///
    /// Err(_)      – RPC / timeout error / Transaction Failed
    ///             - Catch error to view type of error
    pub async fn wait_for_tx(
        &self,
        client: &RpcClient,
        signature: Signature,
        max_wait_seconds: u64,
    ) -> Result<(), TransactionError> {
        let start = std::time::Instant::now();
        loop {
            if start.elapsed().as_secs() > max_wait_seconds {
                return Err(TransactionError::Timeout);
            }

            match client
                .get_signature_statuses_with_history(&[signature])
                .await
            {
                Ok(s) => {
                    if let Some(status) = s.value.get(0).and_then(|x| x.as_ref()) {
                        // Check if transaction failed
                        if status.err.is_some() {
                            return Err(TransactionError::OnChainFailure(format!(
                                "{}",
                                status.err.clone().unwrap()
                            )));
                        }

                        // Check if transaction satisfies the requested commitment level
                        if status.satisfies_commitment(CommitmentConfig {
                            commitment: CommitmentLevel::Finalized,
                        }) {
                            return Ok(());
                        }
                    }
                }
                Err(e) => {
                    // Log the error but continue waiting
                    tracing::warn!("Error checking transaction status: {}", e);
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }

    /// Send solana transaction
    pub async fn send_solana_transaction(
        &self,
        instruction: Instruction,
    ) -> eyre::Result<Signature> {
        let rpc = RpcClient::new_with_timeout_and_commitment(
            self.rpc.clone(),
            Duration::from_secs(60),
            CommitmentConfig {
                commitment: CommitmentLevel::Confirmed,
            },
        );
        let recent = rpc.get_latest_blockhash().await?;
        let payer =
            read_keypair_file(&self.admin_wallet_path).expect("Failed loading solana payer");
        let payer_pubkey = payer.pubkey();
        let tx = Transaction::new(
            &[&payer],
            Message::new(&[instruction], Some(&payer_pubkey)),
            recent,
        );
        let sig = rpc
            .send_transaction_with_config(&tx, RpcSendTransactionConfig {
                skip_preflight: false,
                ..Default::default()
            })
            .await?;

        Ok(sig)
    }

    /// Send solana transaction
    pub async fn send_solana_transaction_inner(
        &self,
        rpc: &RpcClient,
        instruction: Instruction,
    ) -> eyre::Result<Signature, TransactionError> {
        let recent = rpc
            .get_latest_blockhash()
            .await
            .map_err(|e| TransactionError::RpcQueryError(format!("{}", e)))?;
        let payer =
            read_keypair_file(&self.admin_wallet_path).expect("Failed loading solana payer");
        let payer_pubkey = payer.pubkey();
        let tx = Transaction::new(
            &[&payer],
            Message::new(&[instruction], Some(&payer_pubkey)),
            recent,
        );
        let sig = rpc
            .send_transaction_with_config(&tx, RpcSendTransactionConfig {
                skip_preflight: false,
                ..Default::default()
            })
            .await
            .map_err(|e| TransactionError::SendError(format!("{}", e)))?;

        Ok(sig)
    }

    /// Send and confirm solana transaction
    pub async fn send_and_confirm_solana_transaction(
        &self,
        instruction: Instruction,
    ) -> eyre::Result<Signature, TransactionError> {
        let rpc = RpcClient::new_with_timeout_and_commitment(
            self.rpc.clone(),
            Duration::from_secs(60),
            CommitmentConfig {
                commitment: CommitmentLevel::Finalized,
            },
        );
        let sig = self
            .send_solana_transaction_inner(&rpc, instruction)
            .await?;
        self.wait_for_tx(&rpc, sig, 60).await?;

        Ok(sig)
    }
}
