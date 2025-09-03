//! Common utilities and configuration structures for the twine aggregator.
//!
//! This crate provides shared functionality used across the twine aggregator
//! application, including configuration structures and parsing utilities.
use std::fmt;

use serde::{Deserialize, Serialize};
use twine_types::settle::CommitAndFinalizeBatch;

pub mod config;

/// All chains to settle to
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum SettlementChains {
    Ethereum,
    Solana,
}

impl fmt::Display for SettlementChains {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettlementChains::Ethereum => write!(f, "ethereum"),
            SettlementChains::Solana => write!(f, "solana"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub enum DAChains {
    Celestia,
}

impl fmt::Display for DAChains {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DAChains::Celestia => write!(f, "celestia"),
        }
    }
}

/// A trait for querying data from Twine
#[async_trait::async_trait]
pub trait TwineQuery: Send + Sync {
    /// Fetch the data needed to post to celestia
    async fn da_payload(&self, batch_id: u64) -> eyre::Result<Option<Vec<u8>>>;
}

/// A trait for Data Availability layer implementations
#[async_trait::async_trait]
pub trait DALayer: Send + Sync {
    /// Get chain id
    fn chain_id(&self) -> u64;
    /// Get chain name of the da chain
    fn chain_name(&self) -> DAChains;
    /// Post the payload bytes fetched from Twine to the DA network.
    async fn post(&self, payload: &[u8]) -> eyre::Result<()>;
}

/// TransactionStatus
#[derive(Debug, Clone, Default)]
pub struct TransactionStatus {
    /// status
    pub status: bool,
    /// transaction hash
    pub txn_hash: String,
    /// error message
    pub message: Option<String>,
}

/// A trait for Settlement chain implementations
#[async_trait::async_trait]
pub trait SettleBatch: Send + Sync {
    /// Get chain id
    fn chain_id(&self) -> u64;
    /// Chain identifier for settlement chains
    fn chain_name(&self) -> SettlementChains;
    /// Commit and Finalize Transactions
    async fn settle(&self, batch: &CommitAndFinalizeBatch) -> eyre::Result<TransactionStatus>;
    /// Check if a batch is finalized
    async fn is_finalized(&self, batch_id: u64) -> eyre::Result<bool>;
}
