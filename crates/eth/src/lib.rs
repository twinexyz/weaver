//! Wrapper for ethereum reader and writer

pub use {twine_eth_reader, twine_eth_writer};

#[derive(Debug, Clone)]
/// Ethereum Client
pub struct EthClient {
    /// Query ethereum chain
    pub reader: twine_eth_reader::EthReader,
    /// Send transaction to ethereum chain
    pub writer: twine_eth_writer::EthWriter,
}
