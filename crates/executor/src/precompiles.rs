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

#[derive(Clone)]
pub struct TwineCustomPrecompile {
    pub inner: EthPrecompiles,
    pub l1_consensus: Address,
    pub l1_transaction: Address,
}

impl<CTX: ContextTr> PrecompileProvider<CTX> for TwineCustomPrecompile {
    type Output = InterpreterResult;

    fn set_spec(&mut self, spec: <CTX::Cfg as Cfg>::Spec) -> bool {
        self.inner = EthPrecompiles {
            precompiles: Precompiles::prague(),
            spec: spec.into(),
        };
        self.l1_consensus = TWINE_CONSENSUS_VERIFIER_PRECOMPILE_ADDRESS;
        self.l1_transaction = TWINE_TRANSACTION_PRECOMPILE_ADDRESS;
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
            if address.eq(&self.l1_transaction) {
                return TransactionPrecompile::run(context, address, inputs, is_static, gas_limit);
            }
        }

        #[cfg(feature = "twine-l1-consensus-verifier-precompile")]
        {
            if address.eq(&self.l1_consensus) {
                return ConsensusVerifierPrecompile::run(
                    context, address, inputs, is_static, gas_limit,
                );
            }
        }

        self.inner
            .run(context, address, inputs, is_static, gas_limit)
    }

    fn warm_addresses(&self) -> Box<impl Iterator<Item = Address>> { self.inner.warm_addresses() }

    fn contains(&self, address: &Address) -> bool { self.inner.contains(address) }
}
