/// Gas estimator for Ethereum
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GasEstimator {
    ExecuteForcedWithdrawal,
    ExecuteL2Withdraw,
    RefundDeposit,
}

impl GasEstimator {
    pub fn estimate_gas(&self) -> u64 {
        match self {
            Self::ExecuteForcedWithdrawal | Self::ExecuteL2Withdraw | Self::RefundDeposit =>
                200_000,
        }
    }
}
