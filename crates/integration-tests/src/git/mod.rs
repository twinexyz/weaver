use eyre::eyre;
use git2::build::RepoBuilder;
use git2::{Cred, FetchOptions, RemoteCallbacks, Repository, ResetType};

/// Clone a private repository, supporting SSH (local) and HTTPS+token (CI).
pub fn clone_private_repo(url: &str, path: &str) -> Result<Repository, git2::Error> {
    let token = std::env::var("GITHUB_TOKEN").ok();

    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(move |_url, username_from_url, allowed_types| {
        let username = username_from_url.unwrap_or("git");

        // Try SSH agent first
        if allowed_types.is_ssh_key() {
            if let Ok(cred) = Cred::ssh_key_from_agent(username) {
                return Ok(cred);
            }
        }

        // Fallback: HTTPS + Token
        if let Some(ref token) = token {
            if allowed_types.is_user_pass_plaintext() {
                return Cred::userpass_plaintext("git", token);
            }
        }

        Err(git2::Error::from_str(
            "no valid authentication method found",
        ))
    });

    let mut fetch_opts = FetchOptions::new();
    fetch_opts.remote_callbacks(callbacks);

    let mut builder = RepoBuilder::new();
    builder.fetch_options(fetch_opts);

    builder.clone(url, std::path::Path::new(path))
}

pub fn checkout_branch(repo: &Repository, branch_name: &str) -> eyre::Result<()> {
    // Validate branch name (basic check)
    if branch_name.is_empty() {
        return Err(eyre!("Branch does not exist"));
    }

    // Lookup the remote branch
    let branch_ref = format!("origin/{branch_name}");
    let branch_object = repo.revparse_single(&branch_ref)?;
    repo.reset(&branch_object, ResetType::Hard, None)?;

    Ok(())
}

pub fn clone_repository(url: &str, path: &str) -> eyre::Result<Repository> {
    let path = std::path::Path::new(path);
    // Check if path already exists
    if path.exists() {
        // For our testing framework, we'll remove existing directories to allow re-runs
        if path.is_dir() {
            std::fs::remove_dir_all(path)?;
        }
    }

    let repo = Repository::clone(url, path)?;
    Ok(repo)
}

#[test]
#[ignore = "Need to add eval agent before you can run this test"]
/// eval "$(ssh-agent -s)"
/// ssh-add ~/.ssh/private_key  # [which can access private repo `merkora`]
fn test_clone() {
    let ssh = "git@github.com:twinexyz/merkora.git";
    let path = "/tmp/merkora";
    let result = clone_private_repo(ssh, path);
    if let Err(e) = result {
        println!("Error cloning: {e:?}");
    }
}
