//! Wrapper for all EVM Contracts in Twine

use alloy_sol_types::sol;

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    TwineChain,
    "res/TwineChain.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    L2TwineMessenger,
    "res/L2TwineMessenger.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    L1MessageQueue,
    "res/L1MessageQueue.json"
);
