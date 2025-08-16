use alloy_sol_types::{sol, sol_data};

pub(crate) type VerifierInput = (sol_data::Uint<256>, sol_data::Bytes);

pub(crate) type StateRootVerifyParams = (
    sol_data::Uint<64>,
    sol_data::FixedBytes<32>,
    sol_data::Bytes,
    sol_data::Bytes,
);

sol!(
    #[derive(Debug)]
    struct TokenTxn{
        address token;
        address to;
        bool mint;
        uint256 value;
    }

    #[derive(Debug)]
    struct L1Metadata {
        uint64 blockHeight;
        string fromAddress;
        string l1Token;
    }

    #[derive(Debug)]
    struct L1Txns {
        uint64 nonce;
        TokenTxn tokenTxn;
        L1Metadata l1Metadata;
        bytes contractCallData;
    }
);
