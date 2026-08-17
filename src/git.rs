use crate::github::Repo;
use anyhow::{Context, Result};
use git2::{Cred, PushOptions, RemoteCallbacks, Repository, Signature};
use std::env;
use std::path::{Path, PathBuf};

pub fn origin_url(root: &Path) -> Result<String> {
    let repo = Repository::discover(root).context("open git repository")?;
    let remote = repo.find_remote("origin").context("git remote origin")?;
    remote
        .url()
        .context("origin has no URL")
        .map(ToOwned::to_owned)
}

#[must_use]
pub fn upstream_default_branch(root: &Path) -> Option<String> {
    let repo = Repository::discover(root).ok()?;
    let reference = repo.find_reference("refs/remotes/origin/HEAD").ok()?;
    let target = reference.symbolic_target().ok()??;
    let name = target.trim_start_matches("refs/remotes/origin/");
    (!name.is_empty()).then(|| name.to_owned())
}

/// Commit `paths` onto `branch` without moving HEAD. `Empty` if the tree is unchanged.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PushOutcome {
    /// No version files changed.
    Empty,
    /// Branch was updated and pushed.
    Pushed,
}

pub fn commit_branch_and_push(
    root: &Path,
    branch: &str,
    message: &str,
    token: &str,
    github: &Repo,
    paths: &[PathBuf],
) -> Result<PushOutcome> {
    let repo = Repository::discover(root).context("open git repository")?;
    let workdir = repo
        .workdir()
        .context("repository has no workdir")?
        .to_path_buf();
    let parent = repo
        .head()
        .context("git HEAD")?
        .peel_to_commit()
        .context("HEAD commit")?;
    let mut index = repo.index().context("git index")?;
    for path in paths {
        let rel = path.strip_prefix(&workdir).unwrap_or(path);
        if path.exists() {
            index
                .add_path(rel)
                .with_context(|| format!("git add {}", rel.display()))?;
        } else {
            let _ = index.remove_path(rel);
        }
    }
    index.write().context("write index")?;
    let tree_id = index.write_tree().context("write tree")?;
    if parent.tree_id() == tree_id {
        return Ok(PushOutcome::Empty);
    }
    let tree = repo.find_tree(tree_id).context("find tree")?;
    let sig = signature(&repo)?;
    repo.commit(
        Some(&format!("refs/heads/{branch}")),
        &sig,
        &sig,
        message,
        &tree,
        &[&parent],
    )
    .context("git commit")?;

    let https = format!("https://github.com/{}/{}.git", github.owner, github.name);
    let mut remote = repo
        .remote_anonymous(&https)
        .context("anonymous https remote")?;
    let mut callbacks = RemoteCallbacks::new();
    callbacks
        .credentials(|_url, _username, _allowed| Cred::userpass_plaintext("x-access-token", token));
    let mut opts = PushOptions::new();
    opts.remote_callbacks(callbacks);
    let refspec = format!("+refs/heads/{branch}:refs/heads/{branch}");
    remote
        .push(&[&refspec], Some(&mut opts))
        .context("git push")?;
    Ok(PushOutcome::Pushed)
}

fn signature(repo: &Repository) -> Result<Signature<'static>> {
    if let Ok(sig) = repo.signature() {
        return Signature::now(
            sig.name().unwrap_or("verctl"),
            sig.email().unwrap_or("verctl@users.noreply.github.com"),
        )
        .context("commit signature");
    }
    let name = env::var("GIT_AUTHOR_NAME")
        .or_else(|_| env::var("GITHUB_ACTOR"))
        .unwrap_or_else(|_| "verctl".into());
    let email =
        env::var("GIT_AUTHOR_EMAIL").unwrap_or_else(|_| format!("{name}@users.noreply.github.com"));
    Signature::now(&name, &email).context("commit signature")
}

#[cfg(test)]
mod tests {
    use super::origin_url;
    use git2::{Repository, Signature};

    #[test]
    fn origin_url_from_discovered_repo() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        repo.remote(
            "origin",
            "https://github.com/victor-software-house/verctl.git",
        )
        .unwrap();
        let sig = Signature::now("t", "t@example.com").unwrap();
        {
            let mut index = repo.index().unwrap();
            let tid = index.write_tree().unwrap();
            let tree = repo.find_tree(tid).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
                .unwrap();
        }
        assert_eq!(
            origin_url(dir.path()).unwrap(),
            "https://github.com/victor-software-house/verctl.git"
        );
    }

    #[test]
    fn empty_paths_do_not_switch_head() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let sig = Signature::now("t", "t@example.com").unwrap();
        {
            let mut index = repo.index().unwrap();
            let tid = index.write_tree().unwrap();
            let tree = repo.find_tree(tid).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
                .unwrap();
        }
        let before = repo.head().unwrap().name().unwrap().to_owned();
        let repo_id = crate::github::Repo {
            owner: "victor-software-house".into(),
            name: "verctl".into(),
        };
        let out = super::commit_branch_and_push(
            dir.path(),
            "version-packages",
            "chore",
            "unused",
            &repo_id,
            &[],
        )
        .unwrap();
        assert_eq!(out, super::PushOutcome::Empty);
        assert_eq!(repo.head().unwrap().name().unwrap(), before);
    }
}
