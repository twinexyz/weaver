//! Wrapper for all EVM Contracts in Twine

use alloy_sol_types::sol;

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    TwineChain,
    "artifacts/TwineChain.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    L2TwineMessenger,
    "artifacts/L2TwineMessenger.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    TwineSystemStorageContract,
    "artifacts/TwineSystemStorageContract.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    L1MessageQueue,
    "artifacts/L1MessageQueue.json"
);
