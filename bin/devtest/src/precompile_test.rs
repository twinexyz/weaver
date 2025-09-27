use std::fs::{self, File};
use std::str::FromStr;

use alloy_primitives::hex::FromHex;
use alloy_primitives::{address, keccak256, Address, Bytes, FixedBytes, TxKind, B256, U256};
use alloy_provider::{DynProvider, Provider, ProviderBuilder};
use alloy_rpc_types::EIP1186AccountProofResponse;
use alloy_sol_types::{sol, SolCall, SolType};
use eyre::eyre;
use log::info;
use reth_trie_common::AccountProof;
use serde_json::json;
use twine_evm_contracts::l2_twine_messenger::L1Txns;
use twine_l1_eth::twine_l1_eth_writer::transaction::wait_for_receipt;
use twine_l1_eth::twine_l1_eth_writer::EthereumTransaction;
use twine_l1_eth::EthClient;

use crate::precompile_test::PrecompileCaller::L1TransactionsHandled;
use crate::precompile_test::TwineTypes::{MessageData, TransactionType};
use crate::tests::Test;

pub(crate) fn register_precompile_block_production_test() -> Test {
    Test {
        name: "precompile_block_production_test".to_string(),
        description: "Checks that a call to precompile is successful".to_string(),
        test: Box::new(|| Box::pin(precompile_correct_execution())),
    }
}

sol! {
    // SPDX-License-Identifier: MIT
    pragma solidity ^0.8.0;

    interface TwineTypes {
        #[derive(Debug)]
        enum TransactionType {
            Deposit,
            Withdraw,
            Message
        }

        #[derive(Debug)]
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

    #[sol(rpc, bytecode = "0x6080604052348015600e575f5ffd5b50610ec98061001c5f395ff3fe608060405234801561000f575f5ffd5b506004361061003f575f3560e01c80637d41e6cb1461004357806388bf7e771461005f578063ab10445d1461007d575b5f5ffd5b61005d6004803603810190610058919061078a565b610099565b005b6100676101e6565b6040516100749190610865565b60405180910390f35b6100976004803603810190610092919061087e565b6101eb565b005b5f826040015190505f836020015190505f82878787876040516020016100c29493929190610bca565b6040516020818303038152906040526040516020016100e2929190610c2a565b60405160208183030381529060405290505f5f601673ffffffffffffffffffffffffffffffffffffffff168360405161011b9190610c92565b5f604051808303815f865af19150503d805f8114610154576040519150601f19603f3d011682016040523d82523d5f602084013e610159565b606091505b50915091508161019e576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040161019590610d02565b60405180910390fd5b7f6cdc180821b6078572f206bab119a512005185c11b44879e1249c287e1f27715855f86846040516101d39493929190610d9e565b60405180910390a1505050505050505050565b601681565b5f836040015190505f846020015190505f8287878760405160200161021293929190610de8565b604051602081830303815290604052604051602001610232929190610c2a565b60405160208183030381529060405290505f5f601673ffffffffffffffffffffffffffffffffffffffff168360405161026b9190610c92565b5f604051808303815f865af19150503d805f81146102a4576040519150601f19603f3d011682016040523d82523d5f602084013e6102a9565b606091505b5091509150816102ee576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004016102e590610e75565b60405180910390fd5b7f6cdc180821b6078572f206bab119a512005185c11b44879e1249c287e1f27715855f86846040516103239493929190610d9e565b60405180910390a1505050505050505050565b5f604051905090565b5f5ffd5b5f5ffd5b5f819050919050565b61035981610347565b8114610363575f5ffd5b50565b5f8135905061037481610350565b92915050565b5f819050919050565b61038c8161037a565b8114610396575f5ffd5b50565b5f813590506103a781610383565b92915050565b5f5ffd5b5f601f19601f8301169050919050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52604160045260245ffd5b6103f7826103b1565b810181811067ffffffffffffffff82111715610416576104156103c1565b5b80604052505050565b5f610428610336565b905061043482826103ee565b919050565b5f5ffd5b60038110610449575f5ffd5b50565b5f8135905061045a8161043d565b92915050565b5f67ffffffffffffffff82169050919050565b61047c81610460565b8114610486575f5ffd5b50565b5f8135905061049781610473565b92915050565b5f5ffd5b5f5ffd5b5f67ffffffffffffffff8211156104bf576104be6103c1565b5b6104c8826103b1565b9050602081019050919050565b828183375f83830152505050565b5f6104f56104f0846104a5565b61041f565b905082815260208101848484011115610511576105106104a1565b5b61051c8482856104d5565b509392505050565b5f82601f8301126105385761053761049d565b5b81356105488482602086016104e3565b91505092915050565b5f67ffffffffffffffff82111561056b5761056a6103c1565b5b610574826103b1565b9050602081019050919050565b5f61059361058e84610551565b61041f565b9050828152602081018484840111156105af576105ae6104a1565b5b6105ba8482856104d5565b509392505050565b5f82601f8301126105d6576105d561049d565b5b81356105e6848260208601610581565b91505092915050565b5f6101408284031215610605576106046103ad565b5b61061061014061041f565b90505f61061f8482850161044c565b5f83015250602061063284828501610489565b602083015250604061064684828501610489565b604083015250606061065a84828501610489565b606083015250608082013567ffffffffffffffff81111561067e5761067d610439565b5b61068a84828501610524565b60808301525060a082013567ffffffffffffffff8111156106ae576106ad610439565b5b6106ba84828501610524565b60a08301525060c082013567ffffffffffffffff8111156106de576106dd610439565b5b6106ea84828501610524565b60c08301525060e082013567ffffffffffffffff81111561070e5761070d610439565b5b61071a84828501610524565b60e08301525061010082013567ffffffffffffffff81111561073f5761073e610439565b5b61074b84828501610524565b6101008301525061012082013567ffffffffffffffff81111561077157610770610439565b5b61077d848285016105c2565b6101208301525092915050565b5f5f5f5f608085870312156107a2576107a161033f565b5b5f6107af87828801610366565b94505060206107c087828801610399565b935050604085013567ffffffffffffffff8111156107e1576107e0610343565b5b6107ed878288016105ef565b925050606085013567ffffffffffffffff81111561080e5761080d610343565b5b61081a878288016105c2565b91505092959194509250565b5f73ffffffffffffffffffffffffffffffffffffffff82169050919050565b5f61084f82610826565b9050919050565b61085f81610845565b82525050565b5f6020820190506108785f830184610856565b92915050565b5f5f5f5f608085870312156108965761089561033f565b5b5f6108a387828801610399565b945050602085013567ffffffffffffffff8111156108c4576108c3610343565b5b6108d0878288016105ef565b935050604085013567ffffffffffffffff8111156108f1576108f0610343565b5b6108fd878288016105c2565b925050606085013567ffffffffffffffff81111561091e5761091d610343565b5b61092a878288016105c2565b91505092959194509250565b61093f81610347565b82525050565b61094e8161037a565b82525050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52602160045260245ffd5b6003811061099257610991610954565b5b50565b5f8190506109a282610981565b919050565b5f6109b182610995565b9050919050565b6109c1816109a7565b82525050565b6109d081610460565b82525050565b5f81519050919050565b5f82825260208201905092915050565b8281835e5f83830152505050565b5f610a08826109d6565b610a1281856109e0565b9350610a228185602086016109f0565b610a2b816103b1565b840191505092915050565b5f81519050919050565b5f82825260208201905092915050565b5f610a5a82610a36565b610a648185610a40565b9350610a748185602086016109f0565b610a7d816103b1565b840191505092915050565b5f61014083015f830151610a9e5f8601826109b8565b506020830151610ab160208601826109c7565b506040830151610ac460408601826109c7565b506060830151610ad760608601826109c7565b5060808301518482036080860152610aef82826109fe565b91505060a083015184820360a0860152610b0982826109fe565b91505060c083015184820360c0860152610b2382826109fe565b91505060e083015184820360e0860152610b3d82826109fe565b915050610100830151848203610100860152610b5982826109fe565b915050610120830151848203610120860152610b758282610a50565b9150508091505092915050565b5f82825260208201905092915050565b5f610b9c82610a36565b610ba68185610b82565b9350610bb68185602086016109f0565b610bbf816103b1565b840191505092915050565b5f608082019050610bdd5f830187610936565b610bea6020830186610945565b8181036040830152610bfc8185610a88565b90508181036060830152610c108184610b92565b905095945050505050565b610c2481610460565b82525050565b5f604082019050610c3d5f830185610c1b565b8181036020830152610c4f8184610b92565b90509392505050565b5f81905092915050565b5f610c6c82610a36565b610c768185610c58565b9350610c868185602086016109f0565b80840191505092915050565b5f610c9d8284610c62565b915081905092915050565b5f82825260208201905092915050565b7f457468657265756d205472616e73616374696f6e73206661696c6564210000005f82015250565b5f610cec601d83610ca8565b9150610cf782610cb8565b602082019050919050565b5f6020820190508181035f830152610d1981610ce0565b9050919050565b5f819050919050565b5f610d43610d3e610d3984610460565b610d20565b610347565b9050919050565b610d5381610d29565b82525050565b5f819050919050565b5f60ff82169050919050565b5f610d88610d83610d7e84610d59565b610d20565b610d62565b9050919050565b610d9881610d6e565b82525050565b5f608082019050610db15f830187610d4a565b610dbe6020830186610d8f565b610dcb6040830185610d4a565b8181036060830152610ddd8184610b92565b905095945050505050565b5f606082019050610dfb5f830186610945565b8181036020830152610e0d8185610a88565b90508181036040830152610e218184610b92565b9050949350505050565b7f4661696c656420657865637574696e67207472616e73616374696f6e730000005f82015250565b5f610e5f601d83610ca8565b9150610e6a82610e2b565b602082019050919050565b5f6020820190508181035f830152610e8c81610e53565b905091905056fea2646970667358221220ca595a8fddff689713472aae8c06b94eb2ccd63efac5d3b779dfc34818ee2bfd64736f6c634300081b0033")]
    contract PrecompileCaller {
        address public constant PRECOMPILE_ADDRESS = address(0x16);

        event L1TransactionsHandled(
            uint256 chainId,
            uint8 status,
            uint256 nonce,
            bytes transactionOutput
        );

        #[derive(Debug)]
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

            (bool txnSuccess, bytes memory precompileOutput) = PRECOMPILE_ADDRESS.call(
                precompileInput
            );
            require(txnSuccess, "Failed executing transactions");
            emit L1TransactionsHandled(chainId, 0, nonce, precompileOutput);
        }

        #[derive(Debug)]
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
}

async fn precompile_correct_execution() -> eyre::Result<()> {
    let private_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
    let rpc_url = "http://127.0.0.1:8545";

    let eth_writer = EthClient::new(rpc_url, private_key, None).await?;
    let provider = eth_writer.provider.clone();
    info!("Provider setup");

    let deploy_builder = PrecompileCaller::deploy_builder(provider.clone());
    let deploy_tx = deploy_builder.into_transaction_request();
    let deploy_receipt = eth_writer
        .submit_transaction_request_and_wait(deploy_tx)
        .await?;
    let addr = deploy_receipt
        .contract_address
        .ok_or_else(|| eyre!("contract deployment missing address"))?;
    info!("Precompile calling contract deployed at {}", addr);

    info!("Testing ethereum transaction precompile");
    handle_ethereum_precompile(&eth_writer, &addr).await?;
    info!("Testing solana transaction precompile");
    handle_solana_precompile(&eth_writer, &addr).await?;

    Ok(())
}

#[allow(dead_code)]
fn calculate_slot_for_mapping(mapping_slot: U256, inner_key: U256) -> U256 {
    let mut encoded = vec![];
    encoded.extend_from_slice(&inner_key.to_be_bytes_vec());
    encoded.extend_from_slice(&mapping_slot.to_be_bytes_vec());

    U256::from_be_slice(keccak256(&encoded).as_ref())
}

pub async fn handle_ethereum_precompile(
    eth_writer: &EthClient,
    addr: &Address,
) -> eyre::Result<()> {
    let nonce = 533;
    let proof_height: u64 = 9288322;

    // {
    //     let sepolia_rpc = "https://eth-sepolia.public.blastapi.io".parse()?;
    //     let _provider = ProviderBuilder::new().connect_http(sepolia_rpc);
    //     let provider = DynProvider::new(_provider);
    //     let block_height = format!("0x{proof_height:x}");
    //     let slot_position = calculate_slot_for_mapping(U256::from(3),
    //     U256::from(nonce)); let storage_slot: FixedBytes<32> =
    //     slot_position.into(); let block = provider
    //         .get_block_by_number(proof_height.into())
    //         .await?
    //         .unwrap();

    //     let state_root = block.sealed_header().state_root;
    //     let contract = "0x559502369D9B541DA04eFa927D53CD45E9b8F3Ce";
    //     let params = json!([contract.to_string(), [storage_slot], block_height]);

    //     let rpc_response: EIP1186AccountProofResponse =
    //         provider.raw_request("eth_getProof".into(), &params).await?;
    //     let account_proof = AccountProof::from_eip1186_proof(rpc_response);

    //     let mut buf = File::create_new("bin/devtest/res/storage_proof.json")?;
    //     serde_json::to_writer_pretty(&mut buf, &account_proof)?;
    // }

    let state_root =
        B256::from_hex("0xd85f05d2040dfe0d0a73cdc36907f59cf9f71f6c863abc150f2d80eeb4191696")
            .unwrap();
    let reader = fs::read("bin/devtest/res/storage_proof.json")?;
    let account_proof = serde_json::from_slice::<AccountProof>(&reader)?;
    let serialized_proof: Bytes = serde_json::to_vec(&account_proof)?.into();

    let to_address = address!("0x23618e81E3f5cdF7f54C3d65f7FBc0aBf5B21E8f");
    let l2_token = address!("0xF64E5449332820DBC53EA079cC4c3F50ED2491Ed");
    let amount = U256::from(10);
    let message = Bytes::new();

    let message_data = MessageData {
        txnType: TransactionType::Deposit,
        nonce,
        chainId: 11155111,
        blockNumber: 9288321,
        fromAddress: String::from("0x96D9fD19b71E47fDEf2275ccE8Ce06ED2a131241"),
        toAddress: to_address.to_string(),
        l1Token: String::from("0x0000000000000000000000000000000000000000"),
        l2Token: l2_token.to_string(),
        amount: amount.to_string(),
        message: message.clone(),
    };

    let txn_input: Bytes = PrecompileCaller::handleEthereumProofAndTransactionsCall {
        proofHeight: U256::from(proof_height),
        stateRoot: state_root,
        messageData: message_data.clone(),
        serializedProof: serialized_proof,
    }
    .abi_encode()
    .into();

    let chain_id = eth_writer.provider.get_chain_id().await?;

    let tx = EthereumTransaction::new(TxKind::Call(*addr), txn_input, chain_id, 500000, U256::ZERO);

    let receiver = eth_writer.submit_transaction(tx).await?;
    let receipt = wait_for_receipt(receiver).await?;

    let tx_hash = receipt.transaction_hash;
    info!("Ethereum transaction successful: {}", tx_hash);
    for log in receipt.logs() {
        let decoded_log = log.log_decode::<L1TransactionsHandled>()?;
        let l1_txns = L1Txns::abi_decode(&decoded_log.inner.transactionOutput)?;
        let expected_nonce = message_data.nonce;
        assert_eq!(l1_txns.nonce, expected_nonce, "nonce mismatch");
        assert_eq!(l1_txns.tokenTxn.amount, amount, "amount mismatch");
        assert!(l1_txns.tokenTxn.deposit, "deposit flag mismatch");
        assert_eq!(
            l1_txns.tokenTxn.receiver, to_address,
            "receiver address mismatch"
        );
        assert_eq!(l1_txns.tokenTxn.token, l2_token, "l2 token mismatch");
        assert_eq!(
            l1_txns.contractCallData, message,
            "contract call data mismatch"
        );
        let expected_block_number = message_data.blockNumber;
        assert_eq!(
            l1_txns.l1Metadata.blockHeight, expected_block_number,
            "block height mismatch"
        );
        let expected_from_address = message_data.fromAddress.clone();
        assert_eq!(
            l1_txns.l1Metadata.fromAddress, expected_from_address,
            "from address mismatch"
        );
        let expected_l1_token = message_data.l1Token.clone();
        assert_eq!(
            l1_txns.l1Metadata.l1Token, expected_l1_token,
            "expected l1 token mismatch"
        );
    }

    Ok(())
}

pub async fn handle_solana_precompile(eth_writer: &EthClient, addr: &Address) -> eyre::Result<()> {
    let prev_rolling_hash =
        B256::from_hex("0xf48b41bd0b004ce348855d2a43b773f5a4dd8de2c5cce948248e8cf46e3d3d2d")
            .unwrap();
    let public_values = Bytes::from_str("0xddef5a1800000000ddef5a1800000000b103000000000000d86e8112f3c4c4442126f8e9f44f16867da487f29052bf91b810457db34209a4d86e8112f3c4c4442126f8e9f44f16867da487f29052bf91b810457db34209a43c12f6e3a26baeb419633bb411d8770bd59c65bea50b5a57ae83fd5013e77efb000000000000000000000000000000000000000000000000000000000000000000ca9a3b0000000064000000000000000000000001").unwrap();
    let proof = Bytes::new();

    let to_address = address!("0x5BA85D71ef9aE0EdC180a6bf5e65F0a749f062DC");
    let l2_token = address!("0xc1E1CdC77B87391E4B7F25b5C3a70E81C55C99A6");
    let amount = U256::from(1000000000);
    let message = Bytes::new();

    let message_data = MessageData {
        txnType: TransactionType::Deposit,
        nonce: 2,
        chainId: 900,
        blockNumber: 408612829,
        fromAddress: String::from("68AdCXg99JybmZ5VzmDaai5DXS4mqr3gsBf98wM3aANU"),
        toAddress: to_address.to_string(),
        l1Token: String::from("HPuKHQyYfgmq85qyLd8tFF5VW1bekFBrwUm69QHaTQ2W"),
        l2Token: l2_token.to_string(),
        amount: amount.to_string(),
        message: message.clone(),
    };

    let txn_input: Bytes = PrecompileCaller::handleSolanaTransactionsCall {
        prevRollingHash: prev_rolling_hash,
        messageData: message_data.clone(),
        publicValues: public_values,
        proof,
    }
    .abi_encode()
    .into();

    let chain_id = eth_writer.provider.get_chain_id().await?;

    let tx = EthereumTransaction::new(TxKind::Call(*addr), txn_input, chain_id, 500000, U256::ZERO);

    let receiver = eth_writer.submit_transaction(tx).await?;
    let receipt = wait_for_receipt(receiver).await?;

    let tx_hash = receipt.transaction_hash;
    info!("Solana transaction successful: {}", tx_hash);

    let expected_nonce = message_data.nonce;
    let expected_block_number = message_data.blockNumber;
    let expected_from_address = message_data.fromAddress.clone();
    let expected_l1_token = message_data.l1Token.clone();
    for log in receipt.logs() {
        let decoded_log = log.log_decode::<L1TransactionsHandled>()?;
        let l1_txns = L1Txns::abi_decode(&decoded_log.data().transactionOutput)?;
        assert_eq!(l1_txns.nonce, expected_nonce, "nonce mismatch");
        assert_eq!(l1_txns.tokenTxn.amount, amount, "amount mismatch");
        assert!(l1_txns.tokenTxn.deposit, "deposit flag mismatch");
        assert_eq!(
            l1_txns.tokenTxn.receiver, to_address,
            "receiver address mismatch"
        );
        assert_eq!(l1_txns.tokenTxn.token, l2_token, "l2 token mismatch");
        assert_eq!(
            l1_txns.contractCallData, message,
            "contract call data mismatch"
        );
        assert_eq!(
            l1_txns.l1Metadata.blockHeight, expected_block_number,
            "block height mismatch"
        );
        assert_eq!(
            l1_txns.l1Metadata.fromAddress, expected_from_address,
            "from address mismatch"
        );
        assert_eq!(
            l1_txns.l1Metadata.l1Token, expected_l1_token,
            "expected l1 token mismatch"
        );
    }

    Ok(())
}
