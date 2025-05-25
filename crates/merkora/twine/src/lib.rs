use alloy::sol;

pub mod provider;

sol!(
    #[sol(rpc)]
    contract L2Messenger {
        function handleSolanaTransactions(
            uint256 chainId,
            bytes calldata precompileInput
        );

        function handleEthereumProofAndTransactions(
            uint256 chainId,
            uint256 height,
            bytes32 receiptRoot,
            bytes memory consensusProof,
            bytes memory ethereumTransactions
        );
    }
);
