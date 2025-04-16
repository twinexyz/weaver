use alloy_sol_types::{sol, sol_data};

pub(crate) type VerifierInput = (sol_data::Uint<256>, sol_data::Bytes);

pub(crate) type MerkleParamType = (
    sol_data::Array<sol_data::Bytes>,
    sol_data::Array<sol_data::Bytes>,
);

sol!(
    struct TokenTxn{
        address token;
        address to;
        uint256 value;
        bool mint;
    }

    struct L1ForcedTxn{
        address to;
        uint256 value;
        bytes data;
    }

    struct L1Txns {
        uint256 nonce;
        TokenTxn tokenTxn;
        L1ForcedTxn[] forcedTxn;
    }
);
