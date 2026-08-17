use crate::github::Repo;
use anyhow::{Context, Result, bail};
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

/// Fail unless HEAD is on the default-branch history.
///
/// No repository, and no `origin` remote, is a skip — local fixtures
/// have nothing to prove. A repo that has `origin` but no peelable
/// default-tracking ref fails closed. A shallow clone that cannot walk
/// the graph also fails closed.
pub fn require_on_default_history(root: &Path) -> Result<()> {
    let Some(repo) = repo_covering(root) else {
        return Ok(());
    };
    let Ok(head) = repo.head() else {
        return Ok(());
    };
    let Ok(head) = head.peel_to_commit() else {
        return Ok(());
    };
    let Some(upstream) = default_upstream_commit(&repo)? else {
        return Ok(());
    };
    if head.id() == upstream.id() {
        return Ok(());
    }
    match repo.graph_descendant_of(upstream.id(), head.id()) {
        Ok(true) => Ok(()),
        Ok(false) => bail!(
            "publish only from a Version PR commit on the default branch (HEAD {head} is not an ancestor of {upstream})",
            head = head.id(),
            upstream = upstream.id()
        ),
        Err(error) => Err(error).context(
            "publish cannot prove HEAD is on the default branch (need a full fetch, not a shallow clone)",
        ),
    }
}

fn repo_covering(root: &Path) -> Option<Repository> {
    if let Ok(repo) = Repository::open(root) {
        return Some(repo);
    }
    let repo = Repository::discover(root).ok()?;
    let workdir = repo.workdir()?.canonicalize().ok()?;
    let root = root.canonicalize().ok()?;
    root.starts_with(workdir).then_some(repo)
}

fn peel_remote<'a>(repo: &'a Repository, name: &str) -> Option<git2::Commit<'a>> {
    repo.find_reference(name)
        .ok()
        .and_then(|reference| reference.peel_to_commit().ok())
}

fn origin_head_ref(repo: &Repository) -> Option<String> {
    let reference = repo.find_reference("refs/remotes/origin/HEAD").ok()?;
    let target = reference.symbolic_target().ok()??;
    let name = target.trim_start_matches("refs/remotes/origin/");
    (!name.is_empty()).then(|| format!("refs/remotes/origin/{name}"))
}

fn default_branch_names() -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(value) = env::var("GITHUB_BASE_REF") {
        let value = value.trim();
        if !value.is_empty() {
            names.push(value.to_owned());
        }
    }
    if let Ok(value) = env::var("GITHUB_REF_NAME") {
        let value = value.trim();
        // PR events set this to `{n}/merge`. A real branch name does not.
        if !value.is_empty() && !value.contains('/') {
            names.push(value.to_owned());
        }
    }
    for stock in ["main", "master"] {
        if !names.iter().any(|name| name == stock) {
            names.push(stock.to_owned());
        }
    }
    names
}

fn default_upstream_commit(repo: &Repository) -> Result<Option<git2::Commit<'_>>> {
    if let Some(name) = origin_head_ref(repo)
        && let Some(commit) = peel_remote(repo, &name)
    {
        return Ok(Some(commit));
    }
    for name in default_branch_names() {
        let reference = format!("refs/remotes/origin/{name}");
        if let Some(commit) = peel_remote(repo, &reference) {
            return Ok(Some(commit));
        }
    }
    if repo.find_remote("origin").is_ok() {
        bail!(
            "publish cannot prove HEAD is on the default branch (origin exists but origin/HEAD, origin/main, and origin/master are missing; fetch the default branch)"
        );
    }
    Ok(None)
}

/// Fail if the worktree has dirty paths outside `allowed` and `globs`.
pub fn assert_only_allowed(root: &Path, allowed: &[PathBuf], globs: &[String]) -> Result<()> {
    let Ok(repo) = Repository::open(root).or_else(|_| Repository::discover(root)) else {
        return Ok(());
    };
    let statuses = repo.statuses(None).context("git status")?;
    let workdir = repo.workdir().unwrap_or(root);
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let workdir = workdir
        .canonicalize()
        .unwrap_or_else(|_| workdir.to_path_buf());
    let mut extra = Vec::new();
    for entry in statuses.iter() {
        let Ok(rel) = entry.path() else {
            continue;
        };
        let abs = workdir.join(rel);
        if !abs.starts_with(&root) {
            continue;
        }
        if allowed
            .iter()
            .any(|path| path == &abs || path.ends_with(rel))
        {
            continue;
        }
        if globs.iter().any(|pattern| {
            glob::Pattern::new(pattern).is_ok_and(|compiled| {
                compiled.matches(rel) || compiled.matches(&format!("./{rel}"))
            })
        }) {
            continue;
        }
        extra.push(rel.to_owned());
    }
    if extra.is_empty() {
        return Ok(());
    }
    bail!(
        "prepare produced unexpected paths (declare them in [prepare].stage): {}",
        extra.join(", ")
    );
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
    fn unexpected_dirty_fails_unless_globbed() {
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
        std::fs::write(dir.path().join("secret.env"), "TOKEN=leak\n").unwrap();
        let allowed = [dir.path().join("Cargo.toml")];
        let err = super::assert_only_allowed(dir.path(), &allowed, &[]).unwrap_err();
        assert!(format!("{err:#}").contains("secret.env"), "{err:#}");
        super::assert_only_allowed(dir.path(), &allowed, &["*.env".into()]).unwrap();
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

    fn commit_tree(repo: &Repository, message: &str) -> git2::Oid {
        let sig = Signature::now("t", "t@example.com").unwrap();
        let mut index = repo.index().unwrap();
        let tid = index.write_tree().unwrap();
        let tree = repo.find_tree(tid).unwrap();
        let parent = repo.head().ok().and_then(|head| head.peel_to_commit().ok());
        let parents: Vec<&git2::Commit<'_>> = parent.as_ref().into_iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
            .unwrap()
    }

    #[test]
    fn default_history_skips_when_origin_is_missing() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        commit_tree(&repo, "init");
        super::require_on_default_history(dir.path()).unwrap();
    }

    #[test]
    fn head_on_origin_main_is_ok() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let oid = commit_tree(&repo, "init");
        repo.reference("refs/remotes/origin/main", oid, true, "test")
            .unwrap();
        super::require_on_default_history(dir.path()).unwrap();
    }

    #[test]
    fn head_off_default_history_fails() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let first = commit_tree(&repo, "init");
        repo.reference("refs/remotes/origin/main", first, true, "test")
            .unwrap();
        repo.reference("refs/heads/other", first, true, "branch")
            .unwrap();
        repo.set_head("refs/heads/other").unwrap();
        std::fs::write(dir.path().join("extra.txt"), "x\n").unwrap();
        {
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("extra.txt")).unwrap();
            index.write().unwrap();
        }
        commit_tree(&repo, "ahead");
        let err = super::require_on_default_history(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("not an ancestor"), "{err:#}");
    }

    #[test]
    fn origin_without_default_tracking_ref_fails() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        commit_tree(&repo, "init");
        repo.remote(
            "origin",
            "https://github.com/victor-software-house/verctl.git",
        )
        .unwrap();
        let err = super::require_on_default_history(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("origin exists"), "{err:#}");
    }

    #[test]
    fn origin_master_without_head_is_ok() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let oid = commit_tree(&repo, "init");
        repo.reference("refs/remotes/origin/master", oid, true, "test")
            .unwrap();
        super::require_on_default_history(dir.path()).unwrap();
    }

    #[test]
    fn origin_head_to_non_main_is_ok() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let oid = commit_tree(&repo, "init");
        repo.reference("refs/remotes/origin/trunk", oid, true, "test")
            .unwrap();
        repo.reference_symbolic(
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/trunk",
            true,
            "test",
        )
        .unwrap();
        super::require_on_default_history(dir.path()).unwrap();
    }
}
