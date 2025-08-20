//! Wrapper for all EVM Contracts in Twine

pub mod twine_chain {
    use alloy_sol_types::sol;
    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        TwineChain,
        "artifacts/TwineChain.json"
    );
}

pub mod l2_twine_messenger {
    use alloy_sol_types::sol;
    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        L2TwineMessenger,
        "artifacts/L2TwineMessenger.json"
    );
}

pub mod twine_system_storage {
    use alloy_sol_types::sol;
    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        TwineSystemStorage,
        "artifacts/TwineSystemStorage.json"
    );
}

pub mod l1_message_handler {
    use alloy_sol_types::sol;
    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        L1MessageHandler,
        "artifacts/L1MessageHandler.json"
    );
}
