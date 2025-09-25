// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

interface TwineTypes {
    enum TransactionType {
        Deposit,
        Withdraw,
        Message
    }

    struct MessageData {
        TransactionType txnType;
        uint64 nonce;
        uint64 chainId;
        uint64 blockNumber;
        string fromAddress;
        string toAddress;
        string l1Token;
        string l2Token;
        string amount;
        bytes message;
    }
}

contract PrecompileCaller {
    address public constant PRECOMPILE_ADDRESS = address(0x16);

    event L1TransactionsHandled(
        uint256 chainId,
        uint8 status,
        uint256 nonce,
        bytes transactionOutput
    );

    function handleSolanaTransactions(
        bytes32 prevRollingHash,
        TwineTypes.MessageData memory messageData,
        bytes memory publicValues,
        bytes memory proof
    ) external {
        uint64 chainId = messageData.chainId;
        uint64 nonce = messageData.nonce;
        bytes memory precompileInput = abi.encode(
            chainId,
            abi.encode(prevRollingHash, messageData, publicValues)
        );

        (bool txnSuccess, bytes memory precompileOutput) = PRECOMPILE_ADDRESS
            .call(precompileInput);
        require(txnSuccess, "Failed executing transactions");
        emit L1TransactionsHandled(chainId, 0, nonce, precompileOutput);
    }

    function handleEthereumProofAndTransactions(
        uint256 proofHeight,
        bytes32 stateRoot,
        TwineTypes.MessageData memory messageData,
        bytes memory serializedProof
    ) external {
        uint64 chainId = messageData.chainId;
        uint64 nonce = messageData.nonce;
        bytes memory precompile_input = abi.encode(
            chainId,
            abi.encode(proofHeight, stateRoot, messageData, serializedProof)
        );
        (bool txnSuccess, bytes memory precompileOutput) = PRECOMPILE_ADDRESS
            .call(precompile_input);
        require(txnSuccess, "Ethereum Transactions failed!");
        emit L1TransactionsHandled(chainId, 0, nonce, precompileOutput);
    }
}
