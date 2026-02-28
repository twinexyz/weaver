use alloy_evm::precompiles::{DynPrecompile, PrecompileInput, PrecompilesMap};
use alloy_primitives::{Address, Bytes};
use reth::revm::context::{Cfg, ContextTr};
use reth::revm::handler::{EthPrecompiles, PrecompileProvider};
use reth::revm::interpreter::{InputsImpl, InterpreterResult};
use reth::revm::precompile::{PrecompileId, PrecompileOutput, PrecompileResult, Precompiles};
use reth::revm::primitives::hardfork::SpecId;
use twine_constants::precompiles::{
    TWINE_CONSENSUS_VERIFIER_PRECOMPILE_ADDRESS, TWINE_MIDEN_VERIFIER_PRECOMPILE_ADDRESS,
    TWINE_TRANSACTION_PRECOMPILE_ADDRESS, TWINE_ZSTD_PRECOMPILE_ADDRESS,
};
use {
    twine_l1_consensus_verifier_precompile as consensus, twine_l1_transactions_precompile as l1tx,
    twine_miden_verifier_precompile as miden, twine_zstd_precompile as zstd,
};

/// Twine specific precompiles
#[derive(Clone, Debug)]
pub struct TwinePrecompiles {
    pub transaction_precompile: Address,
    pub consensus_precompile: Address,
    pub zstd_precompile: Address,
    pub miden_verifier_precompile: Address,
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

        #[cfg(feature = "twine-zstd-precompile")]
        if self.zstd_precompile.eq(address) {
            return true;
        }

        false
    }

    /// Create a PrecompilesMap with standard Ethereum precompiles and Twine
    /// custom precompiles
    pub fn create_precompiles_map() -> PrecompilesMap {
        // Start with standard Ethereum precompiles
        let mut precompiles = PrecompilesMap::from_static(Precompiles::prague());

        // Add Twine custom precompiles
        #[cfg(feature = "twine-l1-transactions-precompile")]
        {
            let tx: DynPrecompile = (
                PrecompileId::custom("twine_transaction"),
                move |input: PrecompileInput<'_>| -> PrecompileResult {
                    match l1tx::execute(input.data, input.gas) {
                        Ok((bytes, gas_used, reverted)) => Ok(PrecompileOutput {
                            gas_used,
                            bytes,
                            reverted,
                        }),
                        Err(err) => Ok(PrecompileOutput {
                            gas_used: 0,
                            bytes: Bytes::copy_from_slice(err.as_bytes()),
                            reverted: true,
                        }),
                    }
                },
            )
                .into();
            precompiles.apply_precompile(&TWINE_TRANSACTION_PRECOMPILE_ADDRESS, |_| Some(tx));
        }

        #[cfg(feature = "twine-l1-consensus-verifier-precompile")]
        {
            let cons: DynPrecompile = (
                PrecompileId::custom("twine_consensus_verifier"),
                move |input: PrecompileInput<'_>| -> PrecompileResult {
                    match consensus::execute(input.data, input.gas) {
                        Ok((bytes, gas_used, reverted)) => Ok(PrecompileOutput {
                            gas_used,
                            bytes,
                            reverted,
                        }),
                        Err(err) => Ok(PrecompileOutput {
                            gas_used: 0,
                            bytes: Bytes::copy_from_slice(err.as_bytes()),
                            reverted: true,
                        }),
                    }
                },
            )
                .into();
            precompiles
                .apply_precompile(&TWINE_CONSENSUS_VERIFIER_PRECOMPILE_ADDRESS, |_| Some(cons));
        }

        #[cfg(feature = "twine-zstd-precompile")]
        {
            let z: DynPrecompile = (
                PrecompileId::custom("twine_zstd"),
                move |input: PrecompileInput<'_>| -> PrecompileResult {
                    match zstd::execute(input.data, input.gas) {
                        Ok((bytes, gas_used, reverted)) => Ok(PrecompileOutput {
                            gas_used,
                            bytes,
                            reverted,
                        }),
                        Err(err) => Ok(PrecompileOutput {
                            gas_used: 0,
                            bytes: Bytes::copy_from_slice(err.as_bytes()),
                            reverted: true,
                        }),
                    }
                },
            )
                .into();
            precompiles.apply_precompile(&TWINE_ZSTD_PRECOMPILE_ADDRESS, |_| Some(z));
        }

        #[cfg(feature = "twine-miden-verifier-precompile")]
        {
            let z: DynPrecompile = (
                PrecompileId::custom("twine_miden_verifier"),
                move |input: PrecompileInput<'_>| -> PrecompileResult {
                    match miden::execute(input.data, input.gas) {
                        Ok((bytes, gas_used, reverted)) => Ok(PrecompileOutput {
                            gas_used,
                            bytes,
                            reverted,
                        }),
                        Err(err) => Ok(PrecompileOutput {
                            gas_used: 0,
                            bytes: Bytes::copy_from_slice(err.as_bytes()),
                            reverted: true,
                        }),
                    }
                },
            )
                .into();
            precompiles.apply_precompile(&TWINE_ZSTD_PRECOMPILE_ADDRESS, |_| Some(z));
        }

        precompiles
    }

    /// Create a TwineCustomPrecompile that integrates with the EVM context
    /// properly
    pub fn create_twine_precompile_provider() -> TwineCustomPrecompile {
        let eth_precompiles = EthPrecompiles {
            precompiles: Precompiles::prague(),
            spec: SpecId::PRAGUE,
        };

        TwineCustomPrecompile {
            inner: eth_precompiles,
            twine_precompiles: TwinePrecompiles::default(),
        }
    }
}

impl Default for TwinePrecompiles {
    fn default() -> Self {
        Self {
            transaction_precompile: TWINE_TRANSACTION_PRECOMPILE_ADDRESS,
            consensus_precompile: TWINE_CONSENSUS_VERIFIER_PRECOMPILE_ADDRESS,
            zstd_precompile: TWINE_ZSTD_PRECOMPILE_ADDRESS,
            miden_verifier_precompile: TWINE_MIDEN_VERIFIER_PRECOMPILE_ADDRESS,
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
        // No direct calls: precompiles are handled via PrecompilesMap closures now.

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

        #[cfg(feature = "twine-zstd-precompile")]
        let addresses = addresses.chain(std::iter::once(TWINE_ZSTD_PRECOMPILE_ADDRESS));

        Box::new(addresses)
    }

    fn contains(&self, address: &Address) -> bool {
        self.inner.contains(address) || self.twine_precompiles.contains(address)
    }
}
