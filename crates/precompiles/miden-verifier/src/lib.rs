//! Verifier for miden chain

use alloy_primitives::{Bytes, U256};
use alloy_sol_types::{sol_data, SolType, SolValue};
use miden_air::{ExecutionProof, HashFunction, ProcessorAir, ProvingOptions, PublicInputs};
use miden_core::crypto::hash::{Blake3_192, Poseidon2, Rpo256, Rpx256};
use miden_core::crypto::random::{RpoRandomCoin, RpxRandomCoin, WinterRandomCoin};
use miden_core::precompile::{PrecompileVerificationError, PrecompileVerifierRegistry};
use miden_core::utils::Blake3_256;
use miden_core::{ProgramInfo, StackInputs, StackOutputs, Word};
use reth_tracing::tracing;
use winter_verifier::crypto::MerkleTree;
use winter_verifier::{verify as verify_proof, AcceptableOptions, Deserializable, VerifierError};

type MidenVerifierPrecompileInput = (
    sol_data::Bytes,
    sol_data::Bytes,
    sol_data::Bytes,
    sol_data::Bytes,
);

/// Verification of Miden State
pub fn execute(input: &[u8], gas_limit: u64) -> Result<(Bytes, u64, bool), String> {
    
    if gas_limit < 42000 {
        return Err("requires minimum 42000 gas".to_owned());
    }

    let (program_info_bytes, stack_info_bytes, stack_output_bytes, proof_bytes) =
        MidenVerifierPrecompileInput::abi_decode(input).map_err(|e| {
            let error_fmt =
                format!("Failed to deserialize into miden precompile input params. error = {e:?}");
            tracing::error!(error_fmt);
            error_fmt
        })?;
    let program_info =
        ProgramInfo::read_from_bytes(&program_info_bytes).map_err(|e| e.to_string())?;
    let stack_inputs =
        StackInputs::read_from_bytes(&stack_info_bytes).map_err(|e| e.to_string())?;
    let stack_outputs =
        StackOutputs::read_from_bytes(&stack_output_bytes).map_err(|e| e.to_string())?;
    let proof = ExecutionProof::read_from_bytes(&proof_bytes).map_err(|e| e.to_string())?;

    let security_level = match verify(program_info, stack_inputs, stack_outputs, proof) {
        Ok(level) => level,
        Err(err) => {
            tracing::error!(?err, "Miden verification failed");
            return Err(err.to_string());
        }
    };

    let output = U256::from(security_level).abi_encode();
    Ok((output.into(), gas_limit.saturating_sub(42000), true))
}

/// The miden verify function derived from <https://github.com/0xMiden/miden-vm/blob/next/verifier/src/lib.rs#L32>
fn verify(
    program_info: ProgramInfo,
    stack_inputs: StackInputs,
    stack_outputs: StackOutputs,
    proof: ExecutionProof,
) -> Result<u32, VerificationError> {
    let (security_level, _commitment) = verify_with_precompiles(
        program_info,
        stack_inputs,
        stack_outputs,
        proof,
        &PrecompileVerifierRegistry::new(),
    )?;
    Ok(security_level)
}

/// <https://github.com/0xMiden/miden-vm/blob/next/verifier/src/lib.rs#L82>
fn verify_with_precompiles(
    program_info: ProgramInfo,
    stack_inputs: StackInputs,
    stack_outputs: StackOutputs,
    proof: ExecutionProof,
    precompile_verifiers: &PrecompileVerifierRegistry,
) -> Result<(u32, Word), VerificationError> {
    // get security level of the proof
    let security_level = proof.security_level();
    let program_hash = *program_info.program_hash();

    // build public inputs and try to verify the proof
    let pub_inputs = PublicInputs::new(program_info, stack_inputs, stack_outputs);
    let (hash_fn, proof, precompile_requests) = proof.into_parts();

    // TODO: Check that this corresponds to the commitment output by the VM
    // compute the commitment to the list of all precompile requests.
    // if no verifiers were provided (e.g. when this function was called from
    // `verify()`), but the proof contained requests anyway, returns a
    // `NoVerifierFound` error.
    let requests_commitment = precompile_verifiers
        .deferred_requests_commitment(&precompile_requests)
        .map_err(VerificationError::PrecompileVerificationError)?;

    match hash_fn {
        HashFunction::Blake3_192 => {
            let opts = AcceptableOptions::OptionSet(vec![ProvingOptions::REGULAR_96_BITS]);
            verify_proof::<ProcessorAir, Blake3_192, WinterRandomCoin<_>, MerkleTree<_>>(
                proof, pub_inputs, &opts,
            )
        }
        HashFunction::Blake3_256 => {
            let opts = AcceptableOptions::OptionSet(vec![ProvingOptions::REGULAR_128_BITS]);
            verify_proof::<ProcessorAir, Blake3_256, WinterRandomCoin<_>, MerkleTree<_>>(
                proof, pub_inputs, &opts,
            )
        }
        HashFunction::Rpo256 => {
            let opts = AcceptableOptions::OptionSet(vec![
                ProvingOptions::RECURSIVE_96_BITS,
                ProvingOptions::RECURSIVE_128_BITS,
            ]);
            verify_proof::<ProcessorAir, Rpo256, RpoRandomCoin, MerkleTree<_>>(
                proof, pub_inputs, &opts,
            )
        }
        HashFunction::Rpx256 => {
            let opts = AcceptableOptions::OptionSet(vec![
                ProvingOptions::RECURSIVE_96_BITS,
                ProvingOptions::RECURSIVE_128_BITS,
            ]);
            verify_proof::<ProcessorAir, Rpx256, RpxRandomCoin, MerkleTree<_>>(
                proof, pub_inputs, &opts,
            )
        }
        HashFunction::Poseidon2 => {
            let opts = AcceptableOptions::OptionSet(vec![
                ProvingOptions::RECURSIVE_96_BITS,
                ProvingOptions::REGULAR_128_BITS,
            ]);
            verify_proof::<ProcessorAir, Poseidon2, WinterRandomCoin<_>, MerkleTree<_>>(
                proof, pub_inputs, &opts,
            )
        }
    }
    .map_err(|source| VerificationError::ProgramVerificationError(program_hash, source))?;

    Ok((security_level, requests_commitment))
}

#[allow(missing_docs)]
#[derive(Debug, thiserror::Error)]
pub enum VerificationError {
    #[error("failed to verify proof for program with hash {0}")]
    ProgramVerificationError(Word, #[source] VerifierError),
    #[error("the input {0} is not a valid field element")]
    InputNotFieldElement(u64),
    #[error("the output {0} is not a valid field element")]
    OutputNotFieldElement(u64),
    #[error("failed to verify precompile calls")]
    PrecompileVerificationError(#[source] PrecompileVerificationError),
}
