//! zstd compression and decompression precompile

use alloy_primitives::Address;
use alloy_sol_types::{sol, SolCall};
use reth_revm::context::ContextTr;
use reth_revm::interpreter::{Gas, InputsImpl, InstructionResult, InterpreterResult};
use reth_tracing::tracing;
use ruzstd::decoding::StreamingDecoder;
use ruzstd::encoding::{compress_to_vec, CompressionLevel};
use ruzstd::io::Read;

sol! {
    contract ZstdLib {
        function compress(bytes memory data) external view returns(bytes memory);
        function decompress(bytes memory data) external view returns(bytes memory);
    }
}

/// ZSTD Compressor and Decompressor
#[derive(Clone, Debug)]
pub struct ZStdPrecompile;

impl ZStdPrecompile {
    /// Entry point for the zstd precompile execution
    pub fn run<CTX: ContextTr>(
        _context: &mut CTX,
        _address: &Address,
        inputs: &InputsImpl,
        _is_static: bool,
        gas_limit: u64,
    ) -> Result<Option<InterpreterResult>, String> {
        tracing::info!("Zstd precompile invoked");

        if inputs.input.len() < 4 {
            return Err("Invalid Input Length".to_string());
        }

        let selector: &[u8; 4] = inputs.input[..4]
            .try_into()
            .map_err(|_| "Failed to extract 4-byte selector".to_string())?;

        let original = &inputs.input[4..];
        match selector {
            &ZstdLib::compressCall::SELECTOR => {
                tracing::info!("ZSTD Compression");
                let compressed = compress_to_vec(original, CompressionLevel::Fastest);

                tracing::info!(
                    "Original size: {} Compressed size: {} bytes ({}% reduction)",
                    original.len(),
                    compressed.len(),
                    100 - (compressed.len() * 100 / original.len())
                );

                return Ok(Some(InterpreterResult {
                    result: InstructionResult::Return,
                    output: compressed.into(),
                    gas: Gas::new(gas_limit - 21000),
                }));
            }
            &ZstdLib::decompressCall::SELECTOR => {
                tracing::info!("ZSTD Decompression");
                let mut source: &[u8] = &original;
                let mut decoder = StreamingDecoder::new(&mut source).map_err(|e| e.to_string())?;
                let mut result = Vec::new();
                decoder
                    .read_to_end(&mut result)
                    .map_err(|e| e.to_string())?;

                return Ok(Some(InterpreterResult {
                    result: InstructionResult::Return,
                    output: result.into(),
                    gas: Gas::new(gas_limit - 42000),
                }));
            }
            _ => {}
        }

        Err("Invalid Selector".to_string())
    }
}
