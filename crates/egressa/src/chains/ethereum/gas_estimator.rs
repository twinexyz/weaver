pub enum GasEstimator {
    ExecuteForcedWithdrawal,
    ExecuteL2Withdraw,
    RefundDeposit,
}

impl GasEstimator {
    pub fn estimate_gas(&self) -> u64 {
        match self {
            GasEstimator::ExecuteForcedWithdrawal => 200_000,
            GasEstimator::ExecuteL2Withdraw => 200_000,
            GasEstimator::RefundDeposit => 200_000,
        }
    }
}
