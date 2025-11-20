use std::env;

use eyre::Ok;
use log::info;
use twine_devtest::tests;

fn main() -> eyre::Result<()> {
    env_logger::init();

    // Get the first argument from the command line
    let args: Vec<String> = env::args().collect();

    let mut regex_to_match = String::from(".*");
    if args.len() >= 2 {
        let first_arg = &args[1];
        regex_to_match = first_arg.to_string();
        info!("Running tests that match regex: {regex_to_match}");
    } else {
        info!("No regex provided, running all tests");
    }

    let tests = tests::Tests::new();
    tests.run(&regex_to_match)?;
    Ok(())
}
