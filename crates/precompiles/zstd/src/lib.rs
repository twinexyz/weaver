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
                tracing::info!("🍇 Cast into decoder");
                let mut result = Vec::new();
                decoder
                    .read_to_end(&mut result)
                    .map_err(|e| e.to_string())?;
                tracing::info!("🍇 decodingg 🥦");

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

#[cfg(test)]
mod zstd_test {
    use alloy_primitives::hex::FromHex;
    use alloy_primitives::Bytes;
    use ruzstd::decoding::StreamingDecoder;
    use ruzstd::encoding::{compress_to_vec, CompressionLevel};
    use ruzstd::io::Read;

    #[test]
    fn test_zstd_compression_decompression() {
        let original = Bytes::from_hex("0x00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001200000000000000000000000000b306bf915c4d645ff596e518faf3f9669b97016000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000044095ea7b300000000000000000000000094b75aa39bec4cb15e7b9593c315af203b7b847f00000000000000000000000000000000000000000000000000071afd498d00000000000000000000000000000000000000000000000000000000000000000000000000000000000094b75aa39bec4cb15e7b9593c315af203b7b847f00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000060000000000000000000000000000000000000000000000000000000000000010438ed173900000000000000000000000000000000000000000000000000071afd498d0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000007209b5be36e862d9e8e8ccef62407c7184318c11000000000000000000000000000000000000000000000000000000006847034700000000000000000000000000000000000000000000000000000000000000020000000000000000000000000b306bf915c4d645ff596e518faf3f9669b970160000000000000000000000009a9f2ccfde556a7e9ff0848998aa4a0cfd8863ae00000000000000000000000000000000000000000000000000000000").unwrap();
        let compressed = compress_to_vec(&*original.to_vec(), CompressionLevel::Fastest);

        let actual_compressed = Bytes::from_hex("0x28b52ffd04383506000c07000000000000200240010b306bf915c4d645ff596e518faf3f9669b97016006044095ea7b394b75aa39bec4cb15e7b9593c315af203b7b847f071afd498d010438ed1739a07209b5be36e862d9e8e8ccef62407c7184318c11684703479a9f2ccfde556a7e9ff0848998aa4a0cfd8863ae1ba8104f41547f07101c4214830f1012011112634cbc124c34813c5b07ce594ebcce201c72e28cd591d417bf0c91e05cc768778df39f5f826c5434dd09b60781c64cf862e4d998f01818824701e77cc10900fc526971dd").unwrap();
        assert_eq!(actual_compressed.0.to_vec(), compressed);

        let mut compressed_arr: &[u8] = &compressed;
        let mut decoder = StreamingDecoder::new(&mut compressed_arr)
            .map_err(|e| e.to_string())
            .unwrap();
        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .map_err(|e| e.to_string())
            .unwrap();

        assert_eq!(original.0.to_vec(), decompressed);
    }
}
