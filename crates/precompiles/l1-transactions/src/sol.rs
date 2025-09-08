use alloy_sol_types::sol_data;
use twine_evm_contracts::l2_twine_messenger::TwineTypes::MessageData;

pub(crate) type TransactionPrecompileInput = (sol_data::Uint<64>, sol_data::Bytes);

pub(crate) type TransactionPrecompileSolanaInput =
    (sol_data::FixedBytes<32>, MessageData, sol_data::Bytes);

pub(crate) type TransactionPrecompileEthereumInput = (
    sol_data::Uint<256>,
    sol_data::FixedBytes<32>,
    MessageData,
    sol_data::Bytes,
);
