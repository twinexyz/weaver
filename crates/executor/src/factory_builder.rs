use reth::revm::context::TxEnv;
use reth::revm::context_interface::result::{EVMError, HaltReason};
use reth::revm::handler::EthPrecompiles;
use reth::revm::inspector::NoOpInspector;
use reth::revm::interpreter::interpreter::EthInterpreter;
use reth::revm::precompile::Precompiles;
use reth::revm::primitives::hardfork::SpecId;
use reth::revm::{Context, Inspector, MainBuilder, MainContext};
use reth_ethereum::evm::primitives::{Database, EvmEnv};
use reth_ethereum::node::evm::EthEvm;
use reth_evm::eth::EthEvmContext;
use reth_evm::EvmFactory;

use crate::precompiles::{TwineCustomPrecompile, TwinePrecompiles};

#[derive(Debug, Default, Clone)] // removed compy from the original implementation
#[non_exhaustive]
pub struct TwineEvmFactory {
    consensus_verification_target_chains: Vec<String>,
}

impl TwineEvmFactory {
    pub fn new(consensus_verification_target_chains: Vec<String>) -> Self {
        Self {
            consensus_verification_target_chains,
        }
    }
}

impl EvmFactory for TwineEvmFactory {
    type Context<DB: Database> = EthEvmContext<DB>;
    type Error<DBError: core::error::Error + Send + Sync + 'static> = EVMError<DBError>;
    type Evm<DB: Database, I: Inspector<EthEvmContext<DB>, EthInterpreter>> =
        EthEvm<DB, I, TwineCustomPrecompile>;
    type HaltReason = HaltReason;
    type Spec = SpecId;
    type Tx = TxEnv;

    fn create_evm<DB: Database>(&self, db: DB, input: EvmEnv) -> Self::Evm<DB, NoOpInspector> {
        let eth_precompiles = EthPrecompiles {
            precompiles: Precompiles::prague(),
            spec: *input.spec_id(),
        };

        let twine_precompile = TwineCustomPrecompile {
            inner: eth_precompiles,
            twine_precompiles: TwinePrecompiles::default(),
            consensus_verification_target_chains: self.consensus_verification_target_chains.clone(),
        };
        let evm = Context::mainnet()
            .with_db(db)
            .with_cfg(input.cfg_env)
            .with_block(input.block_env)
            .build_mainnet_with_inspector(NoOpInspector {})
            .with_precompiles(twine_precompile);

        EthEvm::new(evm, false)
    }

    fn create_evm_with_inspector<DB: Database, I: Inspector<Self::Context<DB>, EthInterpreter>>(
        &self,
        db: DB,
        input: EvmEnv,
        inspector: I,
    ) -> Self::Evm<DB, I> {
        EthEvm::new(
            self.create_evm(db, input)
                .into_inner()
                .with_inspector(inspector),
            true,
        )
    }
}
