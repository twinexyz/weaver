use alloy_primitives::Address;
use reth::revm::context::{Cfg, ContextTr};
use reth::revm::handler::{EthPrecompiles, PrecompileProvider};
use reth::revm::interpreter::{InputsImpl, InterpreterResult};
use reth::revm::precompile::Precompiles;
use twine_constants::precompiles::{
    TWINE_CONSENSUS_VERIFIER_PRECOMPILE_ADDRESS, TWINE_TRANSACTION_PRECOMPILE_ADDRESS,
};
use twine_l1_consensus_verifier_precompile::ConsensusVerifierPrecompile;
use twine_l1_transactions_precompile::TransactionPrecompile;

/// Twine specific precompiles
#[derive(Clone, Debug)]
pub struct TwinePrecompiles {
    pub transaction_precompile: Address,
    pub consensus_precompile: Address,
}

impl TwinePrecompiles {
    pub fn contains(&self, address: &Address) -> bool {
        #[cfg(feature = "twine-l1-consensus-verifier-precompile")]
        if self.consensus_precompile.eq(address) {
            return true;
        }

        #[cfg(feature = "twine-l1-transactions-precompile")]
        if self.transaction_precompile.eq(address) {
            return true;
        }

        false
    }
}

impl Default for TwinePrecompiles {
    fn default() -> Self {
        Self {
            transaction_precompile: TWINE_TRANSACTION_PRECOMPILE_ADDRESS,
            consensus_precompile: TWINE_CONSENSUS_VERIFIER_PRECOMPILE_ADDRESS,
        }
    }
}

/// General precompiles plus twine precompiles
#[derive(Clone)]
pub struct TwineCustomPrecompile {
    pub inner: EthPrecompiles,
    pub twine_precompiles: TwinePrecompiles,
}

impl<CTX: ContextTr> PrecompileProvider<CTX> for TwineCustomPrecompile {
    type Output = InterpreterResult;

    fn set_spec(&mut self, spec: <CTX::Cfg as Cfg>::Spec) -> bool {
        self.inner = EthPrecompiles {
            precompiles: Precompiles::prague(),
            spec: spec.into(),
        };
        self.twine_precompiles = TwinePrecompiles::default();
        true
    }

    fn run(
        &mut self,
        context: &mut CTX,
        address: &Address,
        inputs: &InputsImpl,
        is_static: bool,
        gas_limit: u64,
    ) -> Result<Option<Self::Output>, String> {
        #[cfg(feature = "twine-l1-transactions-precompile")]
        {
            if address.eq(&self.twine_precompiles.transaction_precompile) {
                return TransactionPrecompile::run(context, address, inputs, is_static, gas_limit);
            }
        }

        #[cfg(feature = "twine-l1-consensus-verifier-precompile")]
        {
            if address.eq(&self.twine_precompiles.consensus_precompile) {
                return ConsensusVerifierPrecompile::run(
                    context, address, inputs, is_static, gas_limit,
                );
            }
        }

        self.inner
            .run(context, address, inputs, is_static, gas_limit)
    }

    fn warm_addresses(&self) -> Box<impl Iterator<Item = Address>> {
        let addresses = self.inner.warm_addresses();

        #[cfg(feature = "twine-l1-consensus-verifier-precompile")]
        let addresses =
            addresses.chain(std::iter::once(TWINE_CONSENSUS_VERIFIER_PRECOMPILE_ADDRESS));

        #[cfg(feature = "twine-l1-transactions-precompile")]
        let addresses = addresses.chain(std::iter::once(TWINE_TRANSACTION_PRECOMPILE_ADDRESS));

        Box::new(addresses)
    }

    fn contains(&self, address: &Address) -> bool {
        self.inner.contains(address) || self.twine_precompiles.contains(address)
    }
}
