//! Wrapper for ethereum reader and writer

use twine_l1_eth_reader::EthReaderBuilder;
use twine_l1_eth_writer::EthWriter;
pub use {twine_l1_eth_reader, twine_l1_eth_writer};

#[derive(Debug, Clone)]
/// Ethereum Client
pub struct EthClient {
    /// Query ethereum chain
    pub reader: twine_l1_eth_reader::EthReader,
    /// Send transaction to ethereum chain
    pub writer: twine_l1_eth_writer::EthWriter,
}

impl EthClient {
    /// Initialize eth client
    pub async fn new(rpc_url: &str, private_key: &str, chain_id: u64) -> eyre::Result<EthClient> {
        let reader_builder = EthReaderBuilder::new().with_execution_rpc(rpc_url);
        let reader = reader_builder.build_with_chain_id(chain_id).await?;
        let writer = EthWriter::new(private_key, rpc_url, Some(chain_id)).await?;
        Ok(EthClient { reader, writer })
    }
}
