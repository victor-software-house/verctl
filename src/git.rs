use crate::github::Repo;
use anyhow::{Context, Result, bail};
use git2::{Cred, PushOptions, RemoteCallbacks, Repository, Signature, Status, StatusOptions};
use std::env;
use std::path::{Path, PathBuf};

#[must_use]
pub fn current_branch(root: &Path) -> Option<String> {
    let repo = repo_covering(root)?;
    let head = repo.head().ok()?;
    let name = head.shorthand().ok()?;
    (name != "HEAD").then(|| name.to_owned())
}

/// The commit HEAD currently names, as a full hex SHA.
pub fn head_sha(root: &Path) -> Result<String> {
    let repo =
        Repository::discover(root).context("publish needs a git repository to name the tag")?;
    let commit = repo
        .head()
        .and_then(|head| head.peel_to_commit())
        .context("publish needs HEAD to name the tag")?;
    Ok(commit.id().to_string())
}

/// File contents at the merge-base of HEAD and the default branch.
///
/// Paths are relative to `root` (the directory holding `.ctl/`) and are mapped
/// onto the git workdir so `-c crates/foo/.ctl/ver.yaml` still hits the
/// right blob. One repository open for the whole list.
pub fn files_on_merge_base(
    root: &Path,
    rels: &[&Path],
    candidates: &[String],
) -> Result<Vec<Option<String>>> {
    let Some(repo) = repo_covering(root) else {
        return Ok(vec![None; rels.len()]);
    };
    let Some(upstream) = default_upstream_commit(&repo, candidates)? else {
        return Ok(vec![None; rels.len()]);
    };
    let head = repo.head().context("cannot compare versions (need HEAD)")?;
    let head = head
        .peel_to_commit()
        .context("cannot compare versions (need HEAD)")?;
    let base = repo.merge_base(head.id(), upstream.id()).context(
        "cannot compare versions against the default branch (need a full fetch, not a shallow clone)",
    )?;
    let tree = repo.find_commit(base)?.tree()?;
    let workdir = repo.workdir().map(Path::to_path_buf);
    Ok(rels
        .iter()
        .map(|rel| {
            git_path(root, rel, workdir.as_deref()).and_then(|path| blob_at(&repo, &tree, &path))
        })
        .collect())
}

fn git_path(root: &Path, rel: &Path, workdir: Option<&Path>) -> Option<PathBuf> {
    let abs = if rel.is_absolute() {
        rel.to_path_buf()
    } else {
        root.join(rel)
    };
    let abs = abs.canonicalize().ok()?;
    let workdir = workdir?.canonicalize().ok()?;
    abs.strip_prefix(workdir).ok().map(Path::to_path_buf)
}

fn blob_at(repo: &Repository, tree: &git2::Tree<'_>, rel: &Path) -> Option<String> {
    let entry = tree.get_path(rel).ok()?;
    let blob = entry.to_object(repo).ok()?.peel_to_blob().ok()?;
    Some(String::from_utf8_lossy(blob.content()).into_owned())
}

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
    prove_default_history(root, &candidate_names(env_base_ref().as_deref()))
}

fn prove_default_history(root: &Path, candidates: &[String]) -> Result<()> {
    let Some(repo) = repo_covering(root) else {
        return Ok(());
    };
    let Ok(head) = repo.head() else {
        return Ok(());
    };
    let Ok(head) = head.peel_to_commit() else {
        return Ok(());
    };
    let Some(upstream) = default_upstream_commit(&repo, candidates)? else {
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

/// The working tree that holds `dir`, canonicalized.
///
/// The ceiling for an upward file search: nothing above a repository
/// root belongs to the project inside it.
#[must_use]
pub fn workdir_covering(dir: &Path) -> Option<PathBuf> {
    repo_covering(dir)?
        .workdir()
        .and_then(|workdir| workdir.canonicalize().ok())
}

/// Every path git tracks under `root`, relative to `root` itself.
///
/// The index, not a directory walk: build output, vendored trees, and
/// anything else gitignored is not part of what this repo publishes. The index
/// speaks in paths from the working tree, and `root` is a project inside it —
/// one repository may hold several — so anything outside `root` is not this
/// project's, and what is left is rebased onto it. A tree with no repository
/// tracks nothing — it cannot serve a file by tag — but a repository whose
/// index cannot be read is an error, never an empty answer.
pub fn tracked(root: &Path) -> Result<Vec<PathBuf>> {
    let Some(repo) = repo_covering(root) else {
        return Ok(Vec::new());
    };
    let index = repo.index().context("git index")?;
    let project = project_prefix(&repo, root);
    Ok(index
        .iter()
        .filter_map(|entry| String::from_utf8(entry.path).ok().map(PathBuf::from))
        .filter_map(|path| path.strip_prefix(&project).ok().map(Path::to_path_buf))
        .collect())
}

/// Whether the repository is told to ignore `rel`, a path relative to `root`.
///
/// Ignoring a file is a repo saying it is not part of what it publishes — the
/// same statement `tracked` already reads the index for. So an ignored file is
/// not a forgotten one, and nothing should ask its author to commit it.
#[must_use]
pub fn is_ignored(root: &Path, rel: &Path) -> bool {
    repo_covering(root).is_some_and(|repo| {
        let project = project_prefix(&repo, root);
        repo.is_path_ignored(project.join(rel)).unwrap_or(false)
    })
}

/// Whether a repository covers `root` at all.
///
/// `tracked` answers "nothing" both for an empty index and for a tree with no
/// repository, and those are different states: a file the index does not carry
/// is untracked only where an index exists to not carry it.
#[must_use]
pub fn is_repository(root: &Path) -> bool {
    repo_covering(root).is_some()
}

/// Where `root` sits inside the working tree: empty when it is the root.
fn project_prefix(repo: &Repository, root: &Path) -> PathBuf {
    let Some(workdir) = repo.workdir().and_then(|dir| dir.canonicalize().ok()) else {
        return PathBuf::new();
    };
    root.canonicalize()
        .ok()
        .and_then(|root| root.strip_prefix(&workdir).ok().map(Path::to_path_buf))
        .unwrap_or_default()
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

fn env_base_ref() -> Option<String> {
    env::var("GITHUB_BASE_REF")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// `GITHUB_BASE_REF` (PR base) then `main` / `master`.
///
/// `GITHUB_REF_NAME` is not a candidate: on push it is the branch
/// being pushed, so `origin/<that branch>` would always match HEAD.
/// A non-main default on Actions is `origin/HEAD`, which
/// `actions/publish` writes from `github.event.repository.default_branch`.
#[must_use]
pub fn default_branch_candidates() -> Vec<String> {
    candidate_names(env_base_ref().as_deref())
}

#[must_use]
pub fn default_branch_candidates_from(base_ref: Option<&str>) -> Vec<String> {
    candidate_names(base_ref)
}

fn candidate_names(base_ref: Option<&str>) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(value) = base_ref.map(str::trim).filter(|value| !value.is_empty()) {
        names.push(value.to_owned());
    }
    for stock in ["main", "master"] {
        if !names.iter().any(|name| name == stock) {
            names.push(stock.to_owned());
        }
    }
    names
}

fn default_upstream_commit<'a>(
    repo: &'a Repository,
    candidates: &[String],
) -> Result<Option<git2::Commit<'a>>> {
    if let Some(name) = origin_head_ref(repo) {
        return match peel_remote(repo, &name) {
            Some(commit) => Ok(Some(commit)),
            None => bail!(
                "cannot resolve the default branch ({name} is missing; fetch the default branch)"
            ),
        };
    }
    for name in candidates {
        let reference = format!("refs/remotes/origin/{name}");
        if let Some(commit) = peel_remote(repo, &reference) {
            return Ok(Some(commit));
        }
    }
    if repo.find_remote("origin").is_ok() {
        bail!(
            "cannot resolve the default branch (origin exists but origin/HEAD, origin/main, and origin/master are missing; fetch the default branch)"
        );
    }
    Ok(None)
}

fn matches_stage(pattern: &str, rel: &str) -> bool {
    glob::Pattern::new(pattern)
        .is_ok_and(|compiled| compiled.matches(rel) || compiled.matches(&format!("./{rel}")))
}

/// Fail if the worktree has dirty paths outside `allowed` and `globs`.
///
/// Returns dirty paths that match `globs` (modified, untracked, or
/// deleted files). Directories are skipped. Same `git status` walk as
/// the unexpected-dirty check; not a filesystem glob.
pub fn assert_only_allowed(
    root: &Path,
    allowed: &[PathBuf],
    globs: &[String],
    stage_ignored: bool,
) -> Result<Vec<PathBuf>> {
    let Ok(repo) = Repository::open(root).or_else(|_| Repository::discover(root)) else {
        return Ok(Vec::new());
    };
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(false);
    let statuses = repo.statuses(Some(&mut opts)).context("git status")?;
    let workdir = repo.workdir().unwrap_or(root);
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let workdir = workdir
        .canonicalize()
        .unwrap_or_else(|_| workdir.to_path_buf());
    let mut extra = Vec::new();
    let mut staged = Vec::new();
    for entry in &statuses {
        let Ok(rel) = entry.path() else {
            continue;
        };
        let abs = workdir.join(rel);
        if !abs.starts_with(&root) {
            continue;
        }
        if abs.is_dir() {
            continue;
        }
        if allowed
            .iter()
            .any(|path| path == &abs || path.ends_with(rel))
        {
            continue;
        }
        if globs.iter().any(|pattern| matches_stage(pattern, rel)) {
            staged.push(abs);
            continue;
        }
        extra.push(rel.to_owned());
    }
    if stage_ignored && !globs.is_empty() {
        let mut ignored = StatusOptions::new();
        ignored
            .include_ignored(true)
            .recurse_ignored_dirs(true)
            .include_untracked(false);
        for pattern in globs {
            ignored.pathspec(pattern.as_str());
        }
        let ignored = repo
            .statuses(Some(&mut ignored))
            .context("git status (ignored)")?;
        for entry in &ignored {
            if !entry.status().intersects(Status::IGNORED) {
                continue;
            }
            let Ok(rel) = entry.path() else {
                continue;
            };
            let abs = workdir.join(rel);
            if !abs.starts_with(&root) || abs.is_dir() {
                continue;
            }
            if globs.iter().any(|pattern| matches_stage(pattern, rel))
                && !staged.iter().any(|path| path == &abs)
            {
                staged.push(abs);
            }
        }
    }
    if extra.is_empty() {
        return Ok(staged);
    }
    bail!(
        "prepare produced unexpected paths (declare them in prepare.stage): {}",
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
        let err = super::assert_only_allowed(dir.path(), &allowed, &[], false).unwrap_err();
        assert!(format!("{err:#}").contains("secret.env"), "{err:#}");
        let staged =
            super::assert_only_allowed(dir.path(), &allowed, &["*.env".into()], false).unwrap();
        assert_eq!(staged.len(), 1);
        assert!(staged[0].ends_with("secret.env"), "{staged:?}");
    }

    #[test]
    fn stage_glob_collects_a_deleted_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let sig = Signature::now("t", "t@example.com").unwrap();
        std::fs::write(dir.path().join("Cargo.lock"), "old\n").unwrap();
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
            index.add_path(Path::new("Cargo.lock")).unwrap();
            index.write().unwrap();
            let tid = index.write_tree().unwrap();
            let tree = repo.find_tree(tid).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
                .unwrap();
        }
        std::fs::remove_file(dir.path().join("Cargo.lock")).unwrap();
        let staged = super::assert_only_allowed(
            dir.path(),
            &[dir.path().join("Cargo.toml")],
            &["Cargo.lock".into()],
            false,
        )
        .unwrap();
        assert_eq!(staged.len(), 1);
        assert!(staged[0].ends_with("Cargo.lock"), "{staged:?}");
        assert!(!dir.path().join("Cargo.lock").exists());
    }

    #[test]
    fn stage_glob_collects_files_under_a_tree_not_the_directory() {
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
            index.write().unwrap();
            let tid = index.write_tree().unwrap();
            let tree = repo.find_tree(tid).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
                .unwrap();
        }
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "fn x() {}\n").unwrap();
        let staged = super::assert_only_allowed(
            dir.path(),
            &[dir.path().join("Cargo.toml")],
            &["src/**".into()],
            false,
        )
        .unwrap();
        assert_eq!(staged.len(), 1);
        assert!(staged[0].ends_with("src/lib.rs"), "{staged:?}");
        assert!(!staged.iter().any(|path| path.ends_with("src")));
    }

    #[test]
    fn ignored_workdir_paths_are_not_unexpected() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let sig = Signature::now("t", "t@example.com").unwrap();
        std::fs::write(dir.path().join(".gitignore"), "/target\n").unwrap();
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
            index.add_path(Path::new(".gitignore")).unwrap();
            index.add_path(Path::new("Cargo.toml")).unwrap();
            index.write().unwrap();
            let tid = index.write_tree().unwrap();
            let tree = repo.find_tree(tid).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
                .unwrap();
        }
        std::fs::create_dir_all(dir.path().join("target/debug")).unwrap();
        std::fs::write(dir.path().join("target/debug/verctl"), "bin\n").unwrap();
        super::assert_only_allowed(dir.path(), &[dir.path().join("Cargo.toml")], &[], false)
            .unwrap();
    }

    #[test]
    fn stage_ignored_opt_in_collects_gitignored_files() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let sig = Signature::now("t", "t@example.com").unwrap();
        std::fs::write(dir.path().join(".gitignore"), "/generated.rs\n").unwrap();
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
            index.add_path(Path::new(".gitignore")).unwrap();
            index.add_path(Path::new("Cargo.toml")).unwrap();
            index.write().unwrap();
            let tid = index.write_tree().unwrap();
            let tree = repo.find_tree(tid).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
                .unwrap();
        }
        std::fs::write(dir.path().join("generated.rs"), "gen\n").unwrap();
        let allowed = [dir.path().join("Cargo.toml")];
        let off = super::assert_only_allowed(dir.path(), &allowed, &["generated.rs".into()], false)
            .unwrap();
        assert!(off.is_empty(), "{off:?}");
        let on = super::assert_only_allowed(dir.path(), &allowed, &["generated.rs".into()], true)
            .unwrap();
        assert_eq!(on.len(), 1);
        assert!(on[0].ends_with("generated.rs"), "{on:?}");
    }

    #[test]
    fn stage_ignored_recurses_a_gitignored_directory() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let sig = Signature::now("t", "t@example.com").unwrap();
        std::fs::write(dir.path().join(".gitignore"), "/dist\n").unwrap();
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
            index.add_path(Path::new(".gitignore")).unwrap();
            index.add_path(Path::new("Cargo.toml")).unwrap();
            index.write().unwrap();
            let tid = index.write_tree().unwrap();
            let tree = repo.find_tree(tid).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
                .unwrap();
        }
        std::fs::create_dir_all(dir.path().join("dist")).unwrap();
        std::fs::write(dir.path().join("dist/out.js"), "ok\n").unwrap();
        let allowed = [dir.path().join("Cargo.toml")];
        let staged =
            super::assert_only_allowed(dir.path(), &allowed, &["dist/**".into()], true).unwrap();
        assert_eq!(staged.len(), 1);
        assert!(staged[0].ends_with("dist/out.js"), "{staged:?}");
    }

    #[test]
    fn stage_ignored_pathspec_does_not_collect_other_ignored_trees() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let sig = Signature::now("t", "t@example.com").unwrap();
        std::fs::write(dir.path().join(".gitignore"), "/dist\n/target\n").unwrap();
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
            index.add_path(Path::new(".gitignore")).unwrap();
            index.add_path(Path::new("Cargo.toml")).unwrap();
            index.write().unwrap();
            let tid = index.write_tree().unwrap();
            let tree = repo.find_tree(tid).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
                .unwrap();
        }
        std::fs::create_dir_all(dir.path().join("dist")).unwrap();
        std::fs::write(dir.path().join("dist/out.js"), "ok\n").unwrap();
        std::fs::create_dir_all(dir.path().join("target/debug")).unwrap();
        std::fs::write(dir.path().join("target/debug/verctl"), "bin\n").unwrap();
        std::fs::write(dir.path().join("target/CACHEDIR.TAG"), "tag\n").unwrap();
        let staged = super::assert_only_allowed(
            dir.path(),
            &[dir.path().join("Cargo.toml")],
            &["dist/**".into()],
            true,
        )
        .unwrap();
        assert_eq!(staged.len(), 1, "{staged:?}");
        assert!(staged[0].ends_with("dist/out.js"), "{staged:?}");
        assert!(
            !staged
                .iter()
                .any(|path| path.to_string_lossy().contains("target")),
            "{staged:?}"
        );
    }

    #[test]
    fn write_commit_on_branch_removes_a_deleted_staged_path() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let sig = Signature::now("t", "t@example.com").unwrap();
        std::fs::write(dir.path().join("Cargo.lock"), "old\n").unwrap();
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
            index.add_path(Path::new("Cargo.lock")).unwrap();
            index.write().unwrap();
            let tid = index.write_tree().unwrap();
            let tree = repo.find_tree(tid).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
                .unwrap();
        }
        std::fs::remove_file(dir.path().join("Cargo.lock")).unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        let workdir = repo.workdir().unwrap().to_path_buf();
        let staged = super::assert_only_allowed(
            dir.path(),
            &[dir.path().join("Cargo.toml")],
            &["Cargo.lock".into()],
            false,
        )
        .unwrap();
        let oid = super::write_commit_on_branch(
            &repo,
            &workdir,
            &parent,
            "version-packages",
            "chore",
            &staged,
        )
        .unwrap()
        .expect("deletion should commit");
        let tree = repo.find_commit(oid).unwrap().tree().unwrap();
        assert!(tree.get_name("Cargo.lock").is_none());
        assert!(tree.get_name("Cargo.toml").is_some());
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

    fn stock() -> Vec<String> {
        super::candidate_names(None)
    }

    fn fixture() -> (tempfile::TempDir, Repository, git2::Oid) {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let oid = commit_tree(&repo, "init");
        (dir, repo, oid)
    }

    fn add_origin(repo: &Repository) {
        repo.remote(
            "origin",
            "https://github.com/victor-software-house/verctl.git",
        )
        .unwrap();
    }

    fn track(repo: &Repository, name: &str, oid: git2::Oid) {
        repo.reference(&format!("refs/remotes/origin/{name}"), oid, true, "test")
            .unwrap();
    }

    fn set_origin_head(repo: &Repository, name: &str) {
        repo.reference_symbolic(
            "refs/remotes/origin/HEAD",
            &format!("refs/remotes/origin/{name}"),
            true,
            "test",
        )
        .unwrap();
    }

    fn prove(root: &std::path::Path, candidates: &[String]) -> anyhow::Result<()> {
        super::prove_default_history(root, candidates)
    }

    #[test]
    fn candidate_names_are_base_ref_then_main_master() {
        assert_eq!(super::candidate_names(None), ["main", "master"]);
        assert_eq!(super::candidate_names(Some("")), ["main", "master"]);
        assert_eq!(super::candidate_names(Some("   ")), ["main", "master"]);
        assert_eq!(
            super::candidate_names(Some("trunk")),
            ["trunk", "main", "master"]
        );
        assert_eq!(super::candidate_names(Some("main")), ["main", "master"]);
        assert_eq!(
            super::candidate_names(Some("release/1.0")),
            ["release/1.0", "main", "master"]
        );
    }

    #[test]
    fn no_repository_skips() {
        let dir = tempfile::TempDir::new().unwrap();
        prove(dir.path(), &stock()).unwrap();
    }

    #[test]
    fn default_history_skips_when_origin_is_missing() {
        let (dir, _, _) = fixture();
        prove(dir.path(), &stock()).unwrap();
    }

    #[test]
    fn head_on_origin_main_is_ok() {
        let (dir, repo, oid) = fixture();
        add_origin(&repo);
        track(&repo, "main", oid);
        prove(dir.path(), &stock()).unwrap();
    }

    #[test]
    fn head_sha_is_the_peeled_head_commit() {
        let (dir, _, oid) = fixture();
        assert_eq!(super::head_sha(dir.path()).unwrap(), oid.to_string());
    }

    #[test]
    fn origin_master_without_head_is_ok() {
        let (dir, repo, oid) = fixture();
        add_origin(&repo);
        track(&repo, "master", oid);
        prove(dir.path(), &stock()).unwrap();
    }

    #[test]
    fn origin_head_to_non_main_is_ok() {
        let (dir, repo, oid) = fixture();
        add_origin(&repo);
        track(&repo, "trunk", oid);
        set_origin_head(&repo, "trunk");
        prove(dir.path(), &stock()).unwrap();
    }

    #[test]
    fn base_ref_trunk_without_origin_head_is_ok() {
        let (dir, repo, oid) = fixture();
        add_origin(&repo);
        track(&repo, "trunk", oid);
        prove(dir.path(), &super::candidate_names(Some("trunk"))).unwrap();
    }

    #[test]
    fn origin_without_default_tracking_ref_fails() {
        let (dir, repo, _) = fixture();
        add_origin(&repo);
        let err = prove(dir.path(), &stock()).unwrap_err();
        assert!(format!("{err:#}").contains("origin exists"), "{err:#}");
    }

    #[test]
    fn origin_trunk_alone_needs_head_or_base_ref() {
        let (dir, repo, oid) = fixture();
        add_origin(&repo);
        track(&repo, "trunk", oid);
        let err = prove(dir.path(), &stock()).unwrap_err();
        assert!(format!("{err:#}").contains("origin exists"), "{err:#}");
        prove(dir.path(), &super::candidate_names(Some("trunk"))).unwrap();
        set_origin_head(&repo, "trunk");
        prove(dir.path(), &stock()).unwrap();
    }

    #[test]
    fn origin_hotfix_alone_is_not_the_default() {
        let (dir, repo, oid) = fixture();
        add_origin(&repo);
        track(&repo, "hotfix", oid);
        let err = prove(dir.path(), &stock()).unwrap_err();
        assert!(format!("{err:#}").contains("origin exists"), "{err:#}");
    }

    #[test]
    fn unpeelable_origin_head_does_not_fall_through_to_main() {
        let (dir, repo, oid) = fixture();
        add_origin(&repo);
        track(&repo, "main", oid);
        set_origin_head(&repo, "trunk");
        let err = prove(dir.path(), &stock()).unwrap_err();
        assert!(format!("{err:#}").contains("origin/trunk"), "{err:#}");
    }

    #[test]
    fn origin_head_wins_over_main_when_they_differ() {
        let (dir, repo, first) = fixture();
        add_origin(&repo);
        std::fs::write(dir.path().join("extra.txt"), "x\n").unwrap();
        {
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("extra.txt")).unwrap();
            index.write().unwrap();
        }
        let second = commit_tree(&repo, "ahead");
        track(&repo, "main", first);
        track(&repo, "trunk", second);
        set_origin_head(&repo, "trunk");
        prove(dir.path(), &stock()).unwrap();
    }

    #[test]
    fn head_off_default_history_fails() {
        let (dir, repo, first) = fixture();
        add_origin(&repo);
        track(&repo, "main", first);
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
        let err = prove(dir.path(), &stock()).unwrap_err();
        assert!(format!("{err:#}").contains("not an ancestor"), "{err:#}");
    }

    #[test]
    fn head_behind_origin_main_is_ok() {
        let (dir, repo, first) = fixture();
        add_origin(&repo);
        std::fs::write(dir.path().join("extra.txt"), "x\n").unwrap();
        {
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("extra.txt")).unwrap();
            index.write().unwrap();
        }
        let second = commit_tree(&repo, "ahead");
        track(&repo, "main", second);
        repo.set_head_detached(first).unwrap();
        prove(dir.path(), &stock()).unwrap();
    }
}
