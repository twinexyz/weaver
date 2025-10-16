use alloy_primitives::{Address, Bytes, U256};
use alloy_rpc_types::TransactionRequest;
use alloy_sol_types::SolCall;
use eyre::Result;
use reth_tracing::tracing::debug;
use twine_evm_contracts::twine_chain::TwineChain;
use twine_l1_eth::twine_l1_eth_reader::clients::execution::EthQueryExecutionClient;

use crate::chains::ethereum::gas_estimator::GasEstimator;
use crate::config::EvmContracts;

#[derive(Debug, Clone)]
/// Transaction builder for Ethereum
pub struct TransactionBuilder {
    /// Query client
    pub query_client: EthQueryExecutionClient,
    /// Contracts configuration
    pub contracts_config: EvmContracts,
    /// Chain ID
    pub chain_id: u64,
}

impl TransactionBuilder {
    /// Create a new transaction builder
    pub fn new(
        query_client: EthQueryExecutionClient,
        contracts_config: EvmContracts,
        chain_id: u64,
    ) -> Self {
        Self {
            query_client,
            contracts_config,
            chain_id,
        }
    }

    /// Parse the twine chain contract address once
    fn get_twine_chain_address(&self) -> Result<Address> {
        self.contracts_config
            .twine_chain_contract
            .parse::<Address>()
            .map_err(|e| eyre::eyre!("Failed to parse twine chain contract address: {}", e))
    }

    /// Prepare unsigned transaction for executeForcedWithdrawal
    pub async fn prepare_execute_forced_withdrawal_transaction(
        &self,
        from: Address,
        public_values: Bytes,
        withdrawal_proof: Bytes,
    ) -> Result<TransactionRequest> {
        let call = TwineChain::executeForcedWithdrawalCall {
            publicValues: public_values,
            withdrawalProof: withdrawal_proof,
        };

        let calldata = call.abi_encode();

        // Get actual nonce and fee estimation from provider
        let nonce = self.query_client.get_nonce(from).await?;
        let (max_fee_per_gas, max_priority_fee_per_gas) =
            self.query_client.get_fee_estimation().await?;

        let contract_address = self.get_twine_chain_address()?;

        let mut tx = TransactionRequest {
            from: Some(from),
            to: Some(contract_address.into()),
            value: Some(U256::from(0)),
            input: calldata.into(),
            nonce: Some(nonce),
            chain_id: Some(self.chain_id),
            max_fee_per_gas: Some(max_fee_per_gas),
            max_priority_fee_per_gas: Some(max_priority_fee_per_gas),
            transaction_type: Some(2), // EIP-1559
            ..Default::default()
        };

        // Try to get actual gas estimation, fallback to hardcoded value
        let gas_limit = self
            .query_client
            .get_gas_estimation(&tx)
            .await
            .unwrap_or_else(|_| GasEstimator::ExecuteForcedWithdrawal.estimate_gas());
        tx.gas = Some(gas_limit);

        debug!(
            "Prepared executeForcedWithdrawal transaction: from={}, chain_id={}, nonce={}, gas_limit={}, max_fee_per_gas={}, max_priority_fee_per_gas={}",
            from, self.chain_id, nonce, gas_limit, max_fee_per_gas, max_priority_fee_per_gas
        );

        Ok(tx)
    }

    /// Prepare unsigned transaction for executeL2Withdraw
    pub async fn prepare_execute_l2_withdraw_transaction(
        &self,
        from: Address,
        public_values: Bytes,
        withdraw_proof: Bytes,
    ) -> Result<TransactionRequest> {
        let call = TwineChain::executeL2WithdrawCall {
            publicValues: public_values,
            withdrawProof: withdraw_proof,
        };

        let calldata = call.abi_encode();

        // Get actual nonce and fee estimation from provider
        let nonce = self.query_client.get_nonce(from).await?;
        let (max_fee_per_gas, max_priority_fee_per_gas) =
            self.query_client.get_fee_estimation().await?;

        let contract_address = self.get_twine_chain_address()?;

        let mut tx = TransactionRequest {
            from: Some(from),
            to: Some(contract_address.into()),
            value: Some(U256::from(0)),
            input: calldata.into(),
            nonce: Some(nonce),
            chain_id: Some(self.chain_id),
            max_fee_per_gas: Some(max_fee_per_gas),
            max_priority_fee_per_gas: Some(max_priority_fee_per_gas),
            transaction_type: Some(2), // EIP-1559
            ..Default::default()
        };

        // Try to get actual gas estimation, fallback to hardcoded value
        let gas_limit = self
            .query_client
            .get_gas_estimation(&tx)
            .await
            .unwrap_or_else(|_| GasEstimator::ExecuteL2Withdraw.estimate_gas());
        tx.gas = Some(gas_limit);

        debug!(
            "Prepared executeL2Withdraw transaction: from={}, chain_id={}, nonce={}, gas_limit={}, max_fee_per_gas={}, max_priority_fee_per_gas={}",
            from, self.chain_id, nonce, gas_limit, max_fee_per_gas, max_priority_fee_per_gas
        );

        Ok(tx)
    }

    /// Prepare unsigned transaction for refundDeposit
    pub async fn prepare_refund_deposit_transaction(
        &self,
        from: Address,
        public_values: Bytes,
        refund_proof: Bytes,
    ) -> Result<TransactionRequest> {
        let call = TwineChain::refundDepositCall {
            publicValues: public_values,
            refundProof: refund_proof,
        };

        let calldata = call.abi_encode();

        // Get actual nonce and fee estimation from provider
        let nonce = self.query_client.get_nonce(from).await?;
        let (max_fee_per_gas, max_priority_fee_per_gas) =
            self.query_client.get_fee_estimation().await?;

        let contract_address = self.get_twine_chain_address()?;

        let mut tx = TransactionRequest {
            from: Some(from),
            to: Some(contract_address.into()),
            value: Some(U256::from(0)),
            input: calldata.into(),
            nonce: Some(nonce),
            chain_id: Some(self.chain_id),
            max_fee_per_gas: Some(max_fee_per_gas),
            max_priority_fee_per_gas: Some(max_priority_fee_per_gas),
            transaction_type: Some(2), // EIP-1559
            ..Default::default()
        };

        // Try to get actual gas estimation, fallback to hardcoded value
        let gas_limit = self
            .query_client
            .get_gas_estimation(&tx)
            .await
            .unwrap_or_else(|_| GasEstimator::RefundDeposit.estimate_gas());
        tx.gas = Some(gas_limit);

        debug!(
            "Prepared refundDeposit transaction: from={}, chain_id={}, nonce={}, gas_limit={}, max_fee_per_gas={}, max_priority_fee_per_gas={}",
            from, self.chain_id, nonce, gas_limit, max_fee_per_gas, max_priority_fee_per_gas
        );

        Ok(tx)
    }
}
