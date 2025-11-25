//! Twine sequencer binary

use clap::Parser;
use tokio;
use tokio::sync::broadcast;
use twine_sequencer::common::shutdown::ShutdownSignal;
use twine_sequencer::config::config::Args;
use twine_sequencer::instance::instance::TwineSequencerInstance;
use twine_sequencer::instance::SequencerInstance;

#[tokio::main]
async fn main() {
    let (kill_sig_sender, mut kill_sig_recv) = broadcast::channel::<ShutdownSignal>(1);

    let cloned_kill_sig_sender = kill_sig_sender.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("error receiving kill signal");
        eprintln!("received ctrl-c");
        cloned_kill_sig_sender
            .send(ShutdownSignal::UserInterrupt)
            .expect("channel send error");
    });

    let args = Args::parse();
    twine_common::logging::init_with_config(None, "twine_nest.log")
        .expect("logging initialization failed");

    let instance = TwineSequencerInstance::new(args)
        .await
        .expect("could not create new sequencer instance");
    instance
        .start(kill_sig_sender)
        .await
        .expect("instance stopped");

    kill_sig_recv
        .recv()
        .await
        .expect("could not receive the kill signal");
}
