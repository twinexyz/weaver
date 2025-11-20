use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use sqlx::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[sqlx(type_name = "on_chain_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum OnChainStatus {
    Pending,
    SendFailed,
    SendSuccessful,
    SendFailedFinalized,
    SendSuccessfulFinalized,
}

impl fmt::Display for OnChainStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Pending => "pending",
            Self::SendFailed => "send_failed",
            Self::SendSuccessful => "send_successful",
            Self::SendFailedFinalized => "send_failed_finalized",
            Self::SendSuccessfulFinalized => "send_successful_finalized",
        };
        write!(f, "{s}")
    }
}

impl FromStr for OnChainStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "send_failed" => Ok(Self::SendFailed),
            "send_successful" => Ok(Self::SendSuccessful),
            "send_failed_finalized" => Ok(Self::SendFailedFinalized),
            "send_successful_finalized" => Ok(Self::SendSuccessfulFinalized),
            _ => Err(format!("Invalid OnChainStatus: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[sqlx(type_name = "da_posting_status_enum", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum DaPostingStatus {
    Pending,
    CommitFailed,
    Committed,
    VerifyFailed,
    Verified,
}

impl fmt::Display for DaPostingStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Pending => "pending",
            Self::CommitFailed => "commit_failed",
            Self::Committed => "committed",
            Self::VerifyFailed => "verify_failed",
            Self::Verified => "verified",
        };
        write!(f, "{s}")
    }
}

impl FromStr for DaPostingStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "commit_failed" => Ok(Self::CommitFailed),
            "committed" => Ok(Self::Committed),
            "verify_failed" => Ok(Self::VerifyFailed),
            "verified" => Ok(Self::Verified),
            _ => Err(format!("Invalid DaPostingStatus: {s}")),
        }
    }
}
