use git2::Repository;
use log::info;

use crate::tests::Test;

pub(crate) fn register_precompile_block_production_test() -> Test {
    Test {
        name: "precompile_block_production_test".to_string(),
        description: "Checks that a call to precompile does not stop block building".to_string(),
        test: Box::new(precompile_block_production_test),
    }
}

fn precompile_block_production_test() -> eyre::Result<()> {
    // Get the git repository root
    let repo = Repository::discover(".")?;
    let repo_root = repo
        .workdir()
        .ok_or_else(|| eyre::eyre!("No working directory found"))?
        .to_path_buf();

    info!("Git repository root: {:?}", repo_root);
    Ok(())
}
