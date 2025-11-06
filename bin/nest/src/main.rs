//! Twine sequencer binary

use clap::Parser;
use tokio;
use twine_sequencer::config::config::Args;
use twine_sequencer::instance::SequencerInstance;

#[tokio::main]
async fn main() {
    let args = Args::parse();
    twine_common::logging::init_with_config(None, "twine_nest.log")
        .expect("logging initialization failed");

    SequencerInstance::start(args).await.unwrap();
}
