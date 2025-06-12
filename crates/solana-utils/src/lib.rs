use alloy_primitives::{Bytes, FixedBytes, U256};
use alloy_sol_types::SolValue;
use async_trait::async_trait;
use borsh::BorshDeserialize;
use tokio::sync::mpsc;
use twine_config::merkora::SolanaConfig;
use twine_constants::pda::{DEPOSIT_PDA_BYTES, WITHDRAW_PDA_BYTES};
use twine_merkora_types::db::{L1MessageDetails, L1MessageType};
use twine_merkora_types::manager::ChainTyp;
use twine_merkora_types::traits::{ChainProvider, ChainTypeHandler};
use twine_merkora_types::{TwineInputParams, PDA};
use twine_solana_consensus_prover_lib::{AccountDeltaProof, PublicValuesStruct};

pub struct SolanaProvider {
    pub cfg: SolanaConfig,
}

impl SolanaProvider {
    pub fn new(cfg: SolanaConfig) -> Self { Self { cfg } }
}

#[async_trait]
impl ChainProvider for SolanaProvider {
    type ProofArtifact = L1MessageDetails;

    async fn accept_consensus_proofs<T>(
        &self,
        chain_details: T,
        tx: mpsc::Sender<Self::ProofArtifact>,
    ) -> eyre::Result<()>
    where
        T: ChainTypeHandler + Send, {
        let consensus_proof = T::get_consensus_proof(&chain_details)?;
        let _public_values = &consensus_proof.public_values;

        let public_values: PublicValuesStruct = serde_json::from_slice(_public_values.as_slice())?;

        let bankhash = match public_values
            .package
            .slot_data
            .iter()
            .max_by_key(|entry| entry.0)
        {
            Some((_, slot_data)) => {
                if let Some(bank_hash) = &slot_data.bank_hash {
                    bank_hash.to_bytes()
                } else {
                    [0; 32] // defaults to zero if no bank hash is found
                }
            }
            None => {
                tracing::error!("No slot data found in public values");
                [0; 32] // defaults to zero if no bank hash is found
            }
        };

        for (_, proofs) in public_values.package.proofs {
            for AccountDeltaProof(pubkey, (account_data, merkle_proof)) in proofs {
                let proof_bytes = serde_json::to_vec(&merkle_proof).map_err(|e| {
                    eyre::eyre!("failed to serialize Merkle proof to a Vec<u8> : {}", e)
                })?;

                // skip first 8 bytes which are appended during anchor serialization
                let mut tx_data = &account_data.account.data[8..];
                let pda: PDA = BorshDeserialize::deserialize(&mut tx_data)?;

                let message_type = match pubkey.to_string() {
                    deposit_key if deposit_key == DEPOSIT_PDA_BYTES => L1MessageType::Deposit,
                    withdraw_key if withdraw_key == WITHDRAW_PDA_BYTES => L1MessageType::Withdraw,
                    unknown => {
                        tracing::error!(?unknown, "unknown PDA pubkey");
                        continue;
                    }
                };

                for msg in pda.messages {
                    let l1_message = L1MessageDetails {
                        nonce: msg.nonce,
                        chain_id: msg.chain_id,
                        block_number: msg.slot_number,
                        message_type: message_type.clone(),
                        receipt_root: bankhash,
                        public_values: _public_values.to_vec(),
                        proof: proof_bytes.clone(),
                    };

                    if let Err(e) = tx.send(l1_message).await {
                        tracing::error!("error sending l1 message details to db: {}", e);
                    }
                }
            }
        }
        Ok(())
    }

    async fn generate_input_params(
        r: L1MessageDetails,
        tx: mpsc::Sender<TwineInputParams>,
    ) -> eyre::Result<()> {
        let receipt_root = FixedBytes::from_slice(&r.receipt_root);
        let svu = generate_verifier_input(U256::from(r.chain_id), r.public_values.into());
        let input_params = TwineInputParams {
            chain_type: ChainTyp::Solana,
            chain_id: r.chain_id,
            nonce: r.nonce,
            account_info: Some(svu.abi_encode_sequence().into()),
            verifier: None,
            transactions: None,
            block_height: Some(r.block_number),
            receipt_root: Some(receipt_root),
        };

        if let Err(e) = tx.send(input_params).await {
            tracing::error!(error = ?e, "solana provider twine input params sender failed");
        }

        Ok(())
    }
}

fn generate_verifier_input(chain_id: U256, bytes: Bytes) -> (U256, Bytes) { (chain_id, bytes) }
