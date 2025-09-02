//! Common utilities and configuration structures for the twine aggregator.
//!
//! This crate provides shared functionality used across the twine aggregator
//! application, including configuration structures and parsing utilities.
use std::fmt;

use serde::{Deserialize, Serialize};

pub mod config;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
