use alloy_sol_types::{sol, sol_data};

pub(crate) type VerifierInput = (sol_data::Uint<256>, sol_data::Bytes);

pub(crate) type MerkleParamType = (
    sol_data::Array<sol_data::Bytes>,
    sol_data::Array<sol_data::Bytes>,
);

sol!(
    #[derive(Debug)]
    struct TokenTxn{
        uint256 value;
        address token;
        address to;
        bool mint;
    }

    #[derive(Debug)]
    struct L1Txns {
        uint256 nonce;
        TokenTxn tokenTxn;
        bytes contractCallData;
    }
);
