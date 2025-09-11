//! Wrapper for all EVM Contracts in Twine

pub mod l2_twine_messenger {
    use alloy_primitives::{keccak256, B256};
    use alloy_sol_types::{sol, SolValue};
    use serde::{Deserialize, Serialize};

    use crate::l2_twine_messenger::TwineTypes::MessageData;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        #[derive(Serialize, Deserialize, Debug)]
        L2TwineMessenger,
        "artifacts/L2TwineMessenger.json"
    );

    sol! {
        #[derive(Serialize, Deserialize, Debug)]
        struct TokenTxn {
            address token;
            address receiver;
            bool deposit;
            uint256 amount;
        }

        #[derive(Serialize, Deserialize, Debug)]
        struct ContractCall {
            address targetContract;
            uint64 value;
            bytes data;
        }

        #[derive(Serialize, Deserialize, Debug)]
        struct L1Metadata {
            uint64 blockHeight;
            string fromAddress;
            string l1Token;
        }

        #[derive(Serialize, Deserialize, Debug)]
        struct L1Txns {
            uint64 nonce;
            TokenTxn tokenTxn;
            L1Metadata l1Metadata;
            bytes contractCallData;
        }

        #[derive(Serialize, Deserialize, Debug)]
        struct MessageDataWithHashedMessage {
            uint8 txn_type;
            uint64 nonce;
            uint64 chainId;
            uint64 blockNumber;
            bytes32 message;
            string fromAddress;
            string toAddress;
            string l1Token;
            string l2Token;
            string amount;
        }
    }

    impl MessageData {
        pub fn hash_message(&self) -> B256 { keccak256(&self.message) }

        pub fn hash_message_data(&self) -> B256 {
            let hashed_message = MessageDataWithHashedMessage {
                txn_type: self.txnType,
                nonce: self.nonce,
                chainId: self.chainId,
                blockNumber: self.blockNumber,
                message: keccak256(&self.message),
                fromAddress: self.fromAddress.to_lowercase().clone(),
                toAddress: self.toAddress.to_lowercase().clone(),
                l1Token: self.l1Token.to_lowercase().clone(),
                l2Token: self.l2Token.to_lowercase().clone(),
                amount: self.amount.to_lowercase().clone(),
            };

            keccak256(hashed_message.abi_encode_packed())
        }
    }
}

pub mod twine_chain {
    use alloy_sol_types::sol;
    use serde::{Deserialize, Serialize};
    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        #[derive(Serialize, Deserialize, Debug)]
        TwineChain,
        "artifacts/TwineChain.json"
    );

    sol! {
        #[derive(Serialize, Deserialize, Debug)]
        enum TransactionType {
            Deposit,
            Withdraw,
            Message,
        }

        #[derive(Serialize, Deserialize, Debug)]
        struct L1OriginTxPublicValues {
            bytes32 batchHash;
            uint64 batchNumber;
            TransactionType txnType;
            uint64 nonce;
            uint64 chainId;
            uint64 blockNumber;
            bytes32 messageHash;
            string fromAddress;
            string toAddress;
            string l1Token;
            string l2Token;
            string amount;
        }

        #[derive(Serialize, Deserialize, Debug)]
        struct L2WithdrawPublicValues {
            uint64 batchNumber;
            uint64 nonce;
            bytes32 batchHash;
            string to;
            string l1Token;
            string l2Token;
            string amount;
        }
    }
}

pub mod twine_system_storage {
    use alloy_sol_types::sol;
    use serde::{Deserialize, Serialize};
    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        #[derive(Serialize, Deserialize, Debug)]
        TwineSystemStorage,
        "artifacts/TwineSystemStorage.json"
    );
}

pub mod l1_message_handler {
    use alloy_sol_types::sol;
    use serde::{Deserialize, Serialize};
    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        #[derive(Serialize, Deserialize, Debug)]
        L1MessageHandler,
        "artifacts/L1MessageHandler.json"
    );
}
