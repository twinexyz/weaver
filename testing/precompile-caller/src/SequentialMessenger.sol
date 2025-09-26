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

contract SequentialMessenger {
    mapping(uint256 => uint256) public lastHandled;

    function getLastMessageExecuted(
        uint256 chainId
    ) external view returns (uint256) {
        return lastHandled[chainId];
    }

    function handleChainTransactions(
        TwineTypes.MessageData memory messageData
    ) external {
        uint256 chain = messageData.chainId;
        uint256 nonce = messageData.nonce;
        uint256 lastHandledNonce = lastHandled[chain];
        require(nonce == lastHandledNonce + 1, "Not correct order");
        lastHandled[chain] = nonce;
    }
}
