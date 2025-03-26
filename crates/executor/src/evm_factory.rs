use std::sync::OnceLock;

use alloy_evm::eth::EthEvmContext;
use alloy_evm::EvmFactory;
use alloy_primitives::{Address, Bytes};
use reth::revm::context::{Cfg, Context, ContextTr, TxEnv};
use reth::revm::context_interface::result::{EVMError, HaltReason};
use reth::revm::handler::{EthPrecompiles, PrecompileProvider};
use reth::revm::inspector::{Inspector, NoOpInspector};
use reth::revm::interpreter::interpreter::EthInterpreter;
use reth::revm::interpreter::InterpreterResult;
use reth::revm::precompile::{PrecompileFn, PrecompileOutput, PrecompileResult, Precompiles};
use reth::revm::primitives::hardfork::SpecId;
use reth::revm::{MainBuilder, MainContext};
use reth_evm::{Database, EvmEnv};
use reth_node_ethereum::evm::EthEvm;

/// Custom EVM configuration.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct TwineEvmFactory;

impl EvmFactory for TwineEvmFactory {
    type Context<DB: Database> = EthEvmContext<DB>;
    type Error<DBError: core::error::Error + Send + Sync + 'static> = EVMError<DBError>;
    type Evm<DB: Database, I: Inspector<EthEvmContext<DB>, EthInterpreter>> =
        EthEvm<DB, I, TwinePrecompiles>;
    type HaltReason = HaltReason;
    type Spec = SpecId;
    type Tx = TxEnv;

    fn create_evm<DB: Database>(&self, db: DB, input: EvmEnv) -> Self::Evm<DB, NoOpInspector> {
        let evm = Context::mainnet()
            .with_db(db)
            .with_cfg(input.cfg_env)
            .with_block(input.block_env)
            .build_mainnet_with_inspector(NoOpInspector {})
            .with_precompiles(TwinePrecompiles::new());

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

/// A custom precompile that contains static precompiles.
#[derive(Clone)]
pub struct TwinePrecompiles {
    pub precompiles: EthPrecompiles,
}

impl TwinePrecompiles {
    /// Given a [`PrecompileProvider`] and cache for a specific precompiles,
    /// create a wrapper that can be used inside Evm.
    fn new() -> Self {
        Self {
            precompiles: EthPrecompiles::default(),
        }
    }
}

/// Returns precompiles for Fjor spec.
pub fn prague_custom() -> &'static Precompiles {
    static INSTANCE: OnceLock<Precompiles> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        let mut precompiles = Precompiles::prague().clone();
        // Custom precompile.
        // precompiles.extend([(
        //     address!("0x0000000000000000000000000000000000000999"),
        //     |_, _| -> PrecompileResult {
        //         PrecompileResult::Ok(PrecompileOutput::new(0, Bytes::new()))
        //     } as PrecompileFn,
        // )
        //     .into()]);

        #[cfg(feature = "twine-consensus-verifier-precompile")]
        precompiles.extend([(
            twine_constants::precompiles::TWINE_CONSENSUS_VERIFIER_PRECOMPILE_ADDRESS,
            |_, _| -> PrecompileResult {
                PrecompileResult::Ok(PrecompileOutput::new(0, Bytes::new()))
            } as PrecompileFn,
        )
            .into()]);
        precompiles
    })
}

impl<CTX: ContextTr> PrecompileProvider<CTX> for TwinePrecompiles {
    type Output = InterpreterResult;

    fn set_spec(&mut self, spec: <CTX::Cfg as Cfg>::Spec) {
        let spec_id = spec.clone().into();
        if spec_id == SpecId::PRAGUE {
            self.precompiles = EthPrecompiles {
                precompiles: prague_custom(),
            }
        } else {
            PrecompileProvider::<CTX>::set_spec(&mut self.precompiles, spec);
        }
    }

    fn run(
        &mut self,
        context: &mut CTX,
        address: &Address,
        bytes: &Bytes,
        gas_limit: u64,
    ) -> Result<Option<Self::Output>, String> {
        self.precompiles.run(context, address, bytes, gas_limit)
    }

    fn warm_addresses(&self) -> Box<impl Iterator<Item = Address>> {
        self.precompiles.warm_addresses()
    }

    fn contains(&self, address: &Address) -> bool { self.precompiles.contains(address) }
}

// /// Custom EVM configuration for Twine.
// #[derive(Debug, Clone)]
// #[non_exhaustive]
// pub struct TwineEvmConfig {
//     pub(crate) inner: EthEvmConfig,
// }

// impl TwineEvmConfig {
//     pub const fn new(chain_spec: Arc<ChainSpec>) -> Self {
//         Self {
//             inner: EthEvmConfig::new(chain_spec),
//         }
//     }

//     fn set_precompiles<EXT, DB>(handler: &mut EvmHandler<EXT, DB>)
//     where
//         DB: reth_evm::Database, {
//         // first we need the evm spec id, which determines the precompiles
//         let spec_id = handler.cfg.spec_id;

//         // install the precompiles
//         handler.pre_execution.load_precompiles = Arc::new(move || {
//             // Suppress `unused_mut` warning in case all the precompiles
//             // are disabled by feature flags.
//             #[allow(unused_mut)]
//             let mut loaded_precompiles =
//
// ContextPrecompiles::new(PrecompileSpecId::from_spec_id(spec_id));

//             #[cfg(feature = "twine-consensus-verifier-precompile")]
//             loaded_precompiles.extend([(
//
// twine_constants::precompiles::TWINE_CONSENSUS_VERIFIER_PRECOMPILE_ADDRESS,
//
// twine_consensus_verifier_precompile::ConsensusVerifierPrecompile::new_ordinary(),
//             )]);

//             // #[cfg(feature = "twine-transactions-precompile")]
//             // loaded_precompiles.extend(transaction_precompile());

//             loaded_precompiles
//         });
//     }
// }

// /// This is implmented for `TwineEvmConfig` because `ConfigureEvm` needs it
// impl ConfigureEvmEnv for TwineEvmConfig {
//     type Error = Infallible;
//     type Header = Header;
//     type Spec = SpecId;
//     type Transaction = TransactionSigned;
//     type TxEnv = TxEnv;

//     fn tx_env(&self, transaction: &Self::Transaction, signer: Address) ->
// Self::TxEnv {         self.inner.tx_env(transaction, signer)
//     }

//     fn evm_env(&self, header: &Self::Header) -> EvmEnv {
// self.inner.evm_env(header) }

//     fn next_evm_env(
//         &self,
//         parent: &Self::Header,
//         attributes: NextBlockEnvAttributes,
//     ) -> Result<EvmEnv, Self::Error> {
//         self.inner.next_evm_env(parent, attributes)
//     }
// }

// impl ConfigureEvm for TwineEvmConfig {
//     type Evm<'a, DB: reth_evm::Database + 'a, I: 'a> = EthEvm<'a, I, DB>;
//     type EvmError<DBError: core::error::Error + Send + Sync + 'static> =
// EVMError<DBError>;     type HaltReason = HaltReason;

//     fn evm_with_env<DB: reth_evm::Database>(
//         &self,
//         db: DB,
//         evm_env: EvmEnv,
//     ) -> Self::Evm<'_, DB, ()> {
//         let cfg_env_with_handler_cfg = CfgEnvWithHandlerCfg {
//             cfg_env: evm_env.cfg_env,
//             handler_cfg: HandlerCfg::new(evm_env.spec),
//         };
//         EvmBuilder::default()
//             .with_db(db)
//             .with_cfg_env_with_handler_cfg(cfg_env_with_handler_cfg)
//             .with_block_env(evm_env.block_env)
//             // add additional precompiles
//             .append_handler_register_box(Box::new(move |handler| {
//                 TwineEvmConfig::set_precompiles(handler)
//             }))
//             .build()
//             .into()
//     }

//     fn evm_with_env_and_inspector<DB, I>(
//         &self,
//         db: DB,
//         evm_env: EvmEnv,
//         inspector: I,
//     ) -> Self::Evm<'_, DB, I>
//     where
//         DB: reth_evm::Database,
//         I: GetInspector<DB>, {
//         let cfg_env_with_handler_cfg = CfgEnvWithHandlerCfg {
//             cfg_env: evm_env.cfg_env,
//             handler_cfg: HandlerCfg::new(evm_env.spec),
//         };
//         EvmBuilder::default()
//             .with_db(db)
//             .with_external_context(inspector)
//             .with_cfg_env_with_handler_cfg(cfg_env_with_handler_cfg)
//             .with_block_env(evm_env.block_env)
//             // add additional precompiles
//             .append_handler_register_box(Box::new(move |handler| {
//                 TwineEvmConfig::set_precompiles(handler)
//             }))
//             .append_handler_register(inspector_handle_register)
//             .build()
//             .into()
//     }
// }
