use std::str::FromStr;
use std::time::Duration;

use alloy_primitives::{Address, Bytes, FixedBytes};
use alloy_provider::{DynProvider, ProviderBuilder, WsConnect};
use alloy_rlp::{RlpDecodable, RlpEncodable};
use alloy_rpc_types::{Block, TransactionReceipt};
use alloy_sol_types::SolType;
use async_trait::async_trait;
use beacon::BeaconProvider;
use eyre::{eyre, Result};
use header::header_to_header;
use sqlx::PgPool;
use ssz::Encode;
use tokio::sync::mpsc;
use transactions::ReceiptsProof;
use twine_config::merkora::EthereumConfig;
use twine_ethereum_consensus_prover_lib::eth::EthPublicValuesStruct;
use twine_types::db::L1MessageDetails;
use twine_types::manager::ChainTyp;
use twine_types::traits::{ChainProvider, ChainTypeHandler};
use twine_types::TwineInputParams;
use utils::TransactionData;

pub mod beacon;
mod header;
pub mod provider;
pub mod stream;
mod transactions;
pub mod utils;

pub use provider::EvmProvider;

pub(crate) const MAX_RETRIES: u32 = 20;
pub(crate) const INITIAL_BACKOFF: Duration = Duration::from_secs(1);

pub struct EthereumContextConfig {
    cfg: EthereumConfig,
}

#[derive(Clone)]
pub struct ContractAddresses {
    // TODO: Add more contract addresses as needed, currently this suffices.
    pub l1_message_queue: Address,
    pub l1_twine_dvn: Address,
}

#[derive(Clone)]
pub struct EthereumContext {
    pub provider: EvmProvider,
    pub db: PgPool,
    pub beacon_provider: BeaconProvider,
    pub chain_id: u64,
    pub chain_type: ChainTyp,
    pub receipt_proofs: ReceiptsProof,
    pub contracts: ContractAddresses,
}

#[derive(Debug, RlpDecodable, RlpEncodable)]
pub struct PrecompileInput {
    pub chain_type: u64,
    pub chain_id: u64,
    pub proof: Vec<Vec<u8>>,
    pub public_inputs: Vec<Vec<u8>>,
    pub participation_mask: Vec<Vec<u8>>,
    pub headers: Vec<alloy_consensus::Header>,
}

impl EthereumContextConfig {
    pub fn new(cfg: EthereumConfig) -> Self { Self { cfg } }

    pub async fn build(&self, db: PgPool) -> EthereumContext {
        let http_rpc_url = self.cfg.rpc.parse().expect("Invalid RPC url");
        let http_provider = ProviderBuilder::new().on_http(http_rpc_url);
        let execution_provider = DynProvider::new(http_provider);

        let beacon_provider = BeaconProvider::new(self.cfg.beacon_rpc.clone());

        let ws_rpc = self.cfg.wss.clone();
        let ws = WsConnect::new(ws_rpc);
        let wss_provider_inner = ProviderBuilder::new().on_ws(ws).await.unwrap();
        let wss_provider = DynProvider::new(wss_provider_inner);

        let l1_message_queue = Address::from_str(&self.cfg.l1_message_queue)
            .expect("Invalid l1 message queue address");
        let l1_twine_dvn =
            Address::from_str(&self.cfg.l1_twine_dvn).expect("Invalid l1 twine dvn address");

        let contracts = ContractAddresses {
            l1_message_queue,
            l1_twine_dvn,
        };
        let provider = EvmProvider::new(execution_provider.clone(), wss_provider.clone());

        EthereumContext {
            provider,
            db,
            beacon_provider,
            chain_id: self.cfg.chain_id,
            chain_type: ChainTyp::Ethereum,
            receipt_proofs: ReceiptsProof::new(l1_message_queue, l1_twine_dvn),
            contracts,
        }
    }
}

#[async_trait]
impl ChainProvider for EthereumContext {
    type ProofArtifact = u64;

    // just need the block number from the proof

    async fn accept_consensus_proofs<T>(
        &self,
        chain_details: T,
        tx: mpsc::Sender<Self::ProofArtifact>,
    ) -> eyre::Result<()>
    where
        T: ChainTypeHandler + Send, {
        tracing::info!("Ethereum consensus proof processing");
        let proof = T::get_consensus_proof(&chain_details)?;
        let sp1_public_inputs = proof.public_values.as_slice();

        let public_input_struct: EthPublicValuesStruct =
            ssz::Decode::from_ssz_bytes(sp1_public_inputs).unwrap(); // FIXME: unwraping for now

        let previous_beacon_block = self
            .beacon_provider
            .get_block_by_number(public_input_struct.previous_block_number)
            .await
            .unwrap();

        let block_number = previous_beacon_block
            .data
            .message
            .body
            .execution_payload
            .block_number;
        tx.send(block_number).await?;
        Ok(())
    }

    async fn generate_input_params(
        r: L1MessageDetails,
        tx: mpsc::Sender<twine_types::TwineInputParams>,
    ) -> eyre::Result<()> {
        let receipt_root = FixedBytes::from_slice(&r.receipt_root);
        let msg_nonce = r.nonce;
        let block_number = r.block_number;
        // TODO: Revisit
        let public_values: Vec<Bytes> = vec![r.public_values.into()];
        let proofs: Vec<Bytes> = vec![r.proof.into()];
        let transactions = Bytes::from(TransactionData::abi_encode_sequence(&(
            public_values,
            proofs,
        )));

        tracing::info!(block_number, msg_nonce, "Sending to twine");

        // Should never fail, because the data loaded to db is deserialized here
        let chain_type = ChainTyp::try_from(r.chain_id).unwrap();

        let input_params = TwineInputParams {
            chain_type,
            chain_id: r.chain_id,
            nonce: msg_nonce,
            account_info: None,
            verifier: Some(Bytes::new()),
            transactions: Some(transactions),
            block_height: Some(block_number),
            receipt_root: Some(receipt_root),
        };

        if let Err(e) = tx.send(input_params).await {
            tracing::error!(error = ?e, "ethereum provider twine input params sender failed");
        }

        Ok(())
    }
}

impl EthereumContext {
    /// This logic is just here for now
    /// On ethereum, we do not generate consensus proof for each block
    /// We generate a proof for a block, and with that, we'll assume
    /// blocks prior that are correct
    /// So, the use case of this function would be to get the height from the
    /// proofs Query all the ethereum messages in DB upto that height
    /// And, send those to chain while running on ZK mode
    #[allow(dead_code)]
    async fn generate_contract_params<T>(
        &self,
        chain_details: T,
        tx: mpsc::Sender<twine_types::TwineInputParams>,
    ) -> eyre::Result<()>
    where
        T: ChainTypeHandler + Send, {
        tracing::info!("Ethereum proof processing");
        let proof = T::get_consensus_proof(&chain_details)?;

        let sp1_public_inputs = proof.public_values.as_slice();

        let public_input_struct: EthPublicValuesStruct =
            ssz::Decode::from_ssz_bytes(sp1_public_inputs).unwrap();

        let beacon_block_number = public_input_struct.beacon_block_number;

        // retry wrapper
        let beacon_block = self
            .beacon_provider
            .get_block_by_number(beacon_block_number)
            .await
            .unwrap(); // TODO dont unwrap

        // retry wrapper
        let previous_beacon_block = self
            .beacon_provider
            .get_block_by_number(public_input_struct.previous_block_number)
            .await
            .unwrap();

        let participation_mask = beacon_block
            .data
            .message
            .body
            .sync_aggregate
            .sync_committee_bits
            .as_ssz_bytes();

        let block_number = previous_beacon_block
            .data
            .message
            .body
            .execution_payload
            .block_number;

        let execution_block = self.get_block_by_number(block_number).await?;
        let receipt_root = execution_block.header.receipts_root;

        let header = header_to_header(execution_block.header);

        let input = PrecompileInput {
            chain_type: 0,
            chain_id: self.chain_id,
            proof: vec![proof.bytes()],
            participation_mask: vec![participation_mask.clone()],
            public_inputs: vec![public_input_struct.as_ssz_bytes()],
            headers: vec![header],
        };

        let verifier_input = Bytes::copy_from_slice(&alloy_rlp::encode(&input));

        let receipts = match self.get_block_receipts(block_number).await {
            Ok(receipt) => receipt,
            Err(_) => {
                tracing::error!("Did not get response from rpc");
                return Err(eyre!("Failed rpc query"));
            }
        };

        let mut encoded_transactions = Bytes::default();

        match self
            .receipt_proofs
            .get_relevant_transactions(block_number, receipts, receipt_root)
        {
            Ok((txns, proofs)) => {
                encoded_transactions =
                    Bytes::from(TransactionData::abi_encode_sequence(&(txns, proofs)));
            }
            Err(e) => {
                tracing::error!(
                    "Error getting deposit and withdraw transactions {}",
                    e.to_string()
                );
            }
        }

        let input_params = TwineInputParams {
            chain_type: self.chain_type.clone(),
            chain_id: self.chain_id,
            // TODO: Parse events
            nonce: 0,
            account_info: None,
            verifier: Some(verifier_input),
            transactions: Some(encoded_transactions),
            receipt_root: Some(receipt_root),
            block_height: Some(block_number),
        };

        tx.send(input_params).await?;

        Ok(())
    }

    // Wrapper functions around the provider methods for cleaner api
    pub async fn get_latest_block(&self) -> Result<u64> { self.provider.get_latest_block().await }

    pub async fn get_block_by_number(&self, height: u64) -> Result<Block> {
        self.provider.get_block_by_number(height).await
    }

    pub async fn get_block_receipts(&self, height: u64) -> Result<Vec<TransactionReceipt>> {
        self.provider.get_block_receipts(height).await
    }
}
