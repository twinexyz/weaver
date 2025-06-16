//! Eth watcher clients

use alloy_rpc_types::Log;

pub mod beacon;
pub mod execution;
pub mod websockets;

// Helper function to sort logs
fn sort_logs(mut logs: Vec<Log>) -> Vec<Log> {
    logs.sort_by(|a, b| {
        a.block_number
            .unwrap_or_default()
            .cmp(&b.block_number.unwrap_or_default())
            .then_with(|| {
                a.log_index
                    .unwrap_or_default()
                    .cmp(&b.log_index.unwrap_or_default())
            })
    });
    logs
}
