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
    let Some(_) = write_commit_on_branch(&repo, &workdir, &parent, branch, message, paths)? else {
        return Ok(PushOutcome::Empty);
    };

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

fn write_commit_on_branch(
    repo: &Repository,
    workdir: &Path,
    parent: &git2::Commit<'_>,
    branch: &str,
    message: &str,
    paths: &[PathBuf],
) -> Result<Option<git2::Oid>> {
    let parent_tree = parent.tree().context("HEAD tree")?;
    let mut index = repo.index().context("git index")?;
    index
        .read_tree(&parent_tree)
        .context("reset index to HEAD")?;
    for path in paths {
        let rel = workdir_rel(workdir, path)?;
        if workdir.join(&rel).exists() {
            index
                .add_path(&rel)
                .with_context(|| format!("git add {}", rel.display()))?;
        } else {
            let _ = index.remove_path(&rel);
        }
    }
    let tree_id = index.write_tree().context("write tree")?;
    if tree_id == parent_tree.id() {
        return Ok(None);
    }
    let tree = repo.find_tree(tree_id).context("find tree")?;
    let sig = signature(repo)?;
    let oid = repo
        .commit(
            Some(&format!("refs/heads/{branch}")),
            &sig,
            &sig,
            message,
            &tree,
            &[parent],
        )
        .context("git commit")?;
    Ok(Some(oid))
}

fn workdir_rel(workdir: &Path, path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workdir.join(path)
    };
    let normalized: PathBuf = absolute
        .components()
        .filter(|component| !matches!(component, std::path::Component::CurDir))
        .collect();
    normalized
        .strip_prefix(workdir)
        .with_context(|| format!("path outside workdir: {}", normalized.display()))
        .map(Path::to_path_buf)
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
    use indoc::indoc;
    use std::path::Path;

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

    #[test]
    fn commit_only_lists_given_paths() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let sig = Signature::now("t", "t@example.com").unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            indoc! {r#"
                [package]
                version = "1.0.0"
            "#},
        )
        .unwrap();
        {
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("Cargo.toml")).unwrap();
            let tid = index.write_tree().unwrap();
            let tree = repo.find_tree(tid).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
                .unwrap();
        }
        std::fs::write(
            dir.path().join("Cargo.toml"),
            indoc! {r#"
                [package]
                version = "1.0.1"
            "#},
        )
        .unwrap();
        std::fs::write(
            dir.path().join("secret.env"),
            indoc! {"
                TOKEN=leak
            "},
        )
        .unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        let oid = super::write_commit_on_branch(
            &repo,
            dir.path(),
            &parent,
            "version-packages",
            "chore",
            &[dir.path().join("Cargo.toml")],
        )
        .unwrap()
        .expect("commit");
        let tree = repo.find_commit(oid).unwrap().tree().unwrap();
        assert!(tree.get_name("Cargo.toml").is_some());
        assert!(tree.get_name("secret.env").is_none());
        assert_eq!(
            repo.head().unwrap().peel_to_commit().unwrap().id(),
            parent.id()
        );
    }

    #[test]
    fn dotted_relative_paths_stage() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let sig = Signature::now("t", "t@example.com").unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            indoc! {r#"
                [package]
                version = "1.0.0"
            "#},
        )
        .unwrap();
        {
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("Cargo.toml")).unwrap();
            let tid = index.write_tree().unwrap();
            let tree = repo.find_tree(tid).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
                .unwrap();
        }
        std::fs::write(
            dir.path().join("Cargo.toml"),
            indoc! {r#"
                [package]
                version = "1.0.1"
            "#},
        )
        .unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        let oid = super::write_commit_on_branch(
            &repo,
            dir.path(),
            &parent,
            "version-packages",
            "chore",
            &[Path::new("./Cargo.toml").to_path_buf()],
        )
        .unwrap()
        .expect("commit");
        assert!(
            repo.find_commit(oid)
                .unwrap()
                .tree()
                .unwrap()
                .get_name("Cargo.toml")
                .is_some()
        );
    }
}
