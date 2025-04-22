use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;

use alloy_consensus::Header;
use alloy_primitives::Address;
use reth::revm::handler::register::EvmHandler;
use reth::revm::precompile::PrecompileSpecId;
use reth::revm::primitives::{
    CfgEnvWithHandlerCfg, EVMError, HaltReason, HandlerCfg, SpecId, TxEnv,
};
use reth::revm::{inspector_handle_register, ContextPrecompiles, EvmBuilder, GetInspector};
use reth_chainspec::ChainSpec;
use reth_evm::{ConfigureEvm, ConfigureEvmEnv, EvmEnv, NextBlockEnvAttributes};
use reth_node_ethereum::evm::EthEvm;
use reth_node_ethereum::EthEvmConfig;
use reth_primitives::TransactionSigned;

/// Custom EVM configuration for Custom.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct TwineEvmConfig {
    pub(crate) inner: EthEvmConfig,
    pub chains_validator_sets: HashMap<u64, String>,
}

impl TwineEvmConfig {
    pub const fn new(
        chain_spec: Arc<ChainSpec>,
        chains_validator_sets: HashMap<u64, String>,
    ) -> Self {
        Self {
            inner: EthEvmConfig::new(chain_spec),
            chains_validator_sets,
        }
    }

    fn set_precompiles<EXT, DB>(
        handler: &mut EvmHandler<EXT, DB>,
        chains_validator_sets: HashMap<u64, String>,
    ) where
        DB: reth_evm::Database, {
        // first we need the evm spec id, which determines the precompiles
        let spec_id = handler.cfg.spec_id;

        // install the precompiles
        handler.pre_execution.load_precompiles = Arc::new(move || {
            // Suppress `unused_mut` warning in case all the precompiles
            // are disabled by feature flags.
            #[allow(unused_mut)]
            let mut loaded_precompiles =
                ContextPrecompiles::new(PrecompileSpecId::from_spec_id(spec_id));

            #[cfg(feature = "twine-l1-consensus-verifier-precompile")]
            loaded_precompiles.extend([(
                twine_constants::precompiles::TWINE_CONSENSUS_VERIFIER_PRECOMPILE_ADDRESS,
                twine_l1_consensus_verifier_precompile::ConsensusVerifierPrecompile::new_ordinary(
                    chains_validator_sets.clone(),
                ),
            )]);

            #[cfg(feature = "twine-l1-transactions-precompile")]
            loaded_precompiles.extend([(
                twine_constants::precompiles::TWINE_TRANSACTION_PRECOMPILE_ADDRESS,
                twine_l1_transactions_precompile::TransactionPrecompile::new_stateful(),
            )]);

            loaded_precompiles
        });
    }
}

/// This is implmented for `TwineEvmConfig` because `ConfigureEvm` needs it
impl ConfigureEvmEnv for TwineEvmConfig {
    type Error = Infallible;
    type Header = Header;
    type Spec = SpecId;
    type Transaction = TransactionSigned;
    type TxEnv = TxEnv;

    fn tx_env(&self, transaction: &Self::Transaction, signer: Address) -> Self::TxEnv {
        self.inner.tx_env(transaction, signer)
    }

    fn evm_env(&self, header: &Self::Header) -> EvmEnv { self.inner.evm_env(header) }

    fn next_evm_env(
        &self,
        parent: &Self::Header,
        attributes: NextBlockEnvAttributes,
    ) -> Result<EvmEnv, Self::Error> {
        self.inner.next_evm_env(parent, attributes)
    }
}

impl ConfigureEvm for TwineEvmConfig {
    type Evm<'a, DB: reth_evm::Database + 'a, I: 'a> = EthEvm<'a, I, DB>;
    type EvmError<DBError: core::error::Error + Send + Sync + 'static> = EVMError<DBError>;
    type HaltReason = HaltReason;

    fn evm_with_env<DB: reth_evm::Database>(
        &self,
        db: DB,
        evm_env: EvmEnv,
    ) -> Self::Evm<'_, DB, ()> {
        let cfg_env_with_handler_cfg = CfgEnvWithHandlerCfg {
            cfg_env: evm_env.cfg_env,
            handler_cfg: HandlerCfg::new(evm_env.spec),
        };
        EvmBuilder::default()
            .with_db(db)
            .with_cfg_env_with_handler_cfg(cfg_env_with_handler_cfg)
            .with_block_env(evm_env.block_env)
            // add additional precompiles
            .append_handler_register_box(Box::new(move |handler| {
                TwineEvmConfig::set_precompiles(handler, self.chains_validator_sets.clone())
            }))
            .build()
            .into()
    }

    fn evm_with_env_and_inspector<DB, I>(
        &self,
        db: DB,
        evm_env: EvmEnv,
        inspector: I,
    ) -> Self::Evm<'_, DB, I>
    where
        DB: reth_evm::Database,
        I: GetInspector<DB>, {
        let cfg_env_with_handler_cfg = CfgEnvWithHandlerCfg {
            cfg_env: evm_env.cfg_env,
            handler_cfg: HandlerCfg::new(evm_env.spec),
        };
        EvmBuilder::default()
            .with_db(db)
            .with_external_context(inspector)
            .with_cfg_env_with_handler_cfg(cfg_env_with_handler_cfg)
            .with_block_env(evm_env.block_env)
            // add additional precompiles
            .append_handler_register_box(Box::new(move |handler| {
                TwineEvmConfig::set_precompiles(handler, self.chains_validator_sets.clone())
            }))
            .append_handler_register(inspector_handle_register)
            .build()
            .into()
    }
}
