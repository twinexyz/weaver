//! sequencer binary
use twine_sequencer::block_progress::progress;

#[tokio::main]
async fn main() { progress().await }
