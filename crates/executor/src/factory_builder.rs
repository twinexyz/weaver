use alloy_evm::precompiles::PrecompilesMap;
use reth::revm::context::TxEnv;
use reth::revm::context_interface::result::{EVMError, HaltReason};
use reth::revm::inspector::NoOpInspector;
use reth::revm::interpreter::interpreter::EthInterpreter;
use reth::revm::primitives::hardfork::SpecId;
use reth::revm::{Context, Inspector, MainBuilder, MainContext};
use reth_ethereum::evm::primitives::{Database, EvmEnv};
use reth_ethereum::node::evm::EthEvm;
use reth_evm::eth::EthEvmContext;
use reth_evm::EvmFactory;

use crate::precompiles::TwinePrecompiles;

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct TwineEvmFactory {
    precompiles: PrecompilesMap,
}

impl TwineEvmFactory {
    pub fn new() -> Self {
        Self {
            precompiles: TwinePrecompiles::create_precompiles_map(),
        }
    }
}

impl EvmFactory for TwineEvmFactory {
    type Context<DB: Database> = EthEvmContext<DB>;
    type Error<DBError: core::error::Error + Send + Sync + 'static> = EVMError<DBError>;
    type Evm<DB: Database, I: Inspector<EthEvmContext<DB>, EthInterpreter>> =
        EthEvm<DB, I, PrecompilesMap>;
    type HaltReason = HaltReason;
    type Precompiles = PrecompilesMap;
    type Spec = SpecId;
    type Tx = TxEnv;

    fn create_evm<DB: Database>(&self, db: DB, input: EvmEnv) -> Self::Evm<DB, NoOpInspector> {
        reth_tracing::tracing::info!("TwineEvmFactory: Creating EVM with custom precompiles");

        let inner = Context::mainnet()
            .with_db(db)
            .with_cfg(input.cfg_env)
            .with_block(input.block_env)
            .build_mainnet_with_inspector(NoOpInspector {})
            .with_precompiles(self.precompiles.clone());

        EthEvm::new(inner, false)
    }

    fn create_evm_with_inspector<DB: Database, I: Inspector<Self::Context<DB>, EthInterpreter>>(
        &self,
        db: DB,
        input: EvmEnv,
        inspector: I,
    ) -> Self::Evm<DB, I> {
        reth_tracing::tracing::info!(
            "TwineEvmFactory: Creating EVM with inspector and custom precompiles"
        );

        let inner = Context::mainnet()
            .with_db(db)
            .with_cfg(input.cfg_env)
            .with_block(input.block_env)
            .build_mainnet_with_inspector(inspector)
            .with_precompiles(self.precompiles.clone());

        EthEvm::new(inner, true)
    }
}
