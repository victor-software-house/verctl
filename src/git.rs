use std::collections::BTreeSet;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};
use gix::bstr::ByteSlice;
use gix::credentials::helper::Action;
use gix::credentials::protocol::Outcome;
use gix::objs::tree::EntryKind;
use gix::refs::Target;
use gix::refs::transaction::{PreviousValue, RefEdit};
use gix::remote::Direction;
use gix::sec::identity::Account;
use gix::sec::trust::{DefaultForLevel, Mapping};
use gix::sec::{Permission, Trust};

use crate::github::Repo;
use crate::process;

/// The checked-out branch, or `None` for a detached HEAD or no repository.
#[must_use]
pub fn current_branch(root: &Path) -> Option<String> {
    let repo = repo_covering(root)?;
    repo.head_id().ok()?;
    let name = repo.head_name().ok()??;
    Some(name.shorten().to_string())
}

/// The commit HEAD currently names, as a full hex SHA.
pub fn head_sha(root: &Path) -> Result<String> {
    let repo = discover(root).context("publish needs a git repository to name the tag")?;
    let commit = repo
        .head_commit()
        .context("publish needs HEAD to name the tag")?;
    Ok(commit.id().to_string())
}

/// What a missing or unreachable merge-base means for a version comparison.
const NEED_FULL_FETCH: &str =
    "cannot compare versions against the default branch (need a full fetch, not a shallow clone)";

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
    let head = repo
        .head_commit()
        .context("cannot compare versions (need HEAD)")?;
    let base = repo
        .merge_base(head.id(), upstream)
        .context(NEED_FULL_FETCH)?
        .context(NEED_FULL_FETCH)?;
    let tree = repo.find_commit(base)?.tree()?;
    let workdir = repo.workdir().map(Path::to_path_buf);
    Ok(rels
        .iter()
        .map(|rel| git_path(root, rel, workdir.as_deref()).and_then(|path| blob_at(&tree, &path)))
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

fn blob_at(tree: &gix::Tree<'_>, rel: &Path) -> Option<String> {
    let entry = tree.lookup_entry_by_path(rel).ok()??;
    let blob = entry.object().ok()?.try_into_blob().ok()?;
    Some(String::from_utf8_lossy(&blob.data).into_owned())
}

/// The URL of the `origin` remote.
pub fn origin_url(root: &Path) -> Result<String> {
    let repo = discover(root).context("open git repository")?;
    let remote = repo.find_remote("origin").context("git remote origin")?;
    let url = remote.url(Direction::Fetch).context("origin has no URL")?;
    Ok(url.to_bstring().to_str_lossy().into_owned())
}

/// The commit that added a file, as the changelog names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddedBy {
    /// Full hex SHA of the commit.
    pub sha: String,
    /// First line of the commit message.
    pub subject: String,
}

/// The commit that added `path`: the newest commit on HEAD's first-parent
/// history whose tree has the file and whose first parent's tree does not. A
/// squash merge is that commit, and its subject carries the pull request
/// number. When that commit is a merge and another parent already has the
/// file, the file arrived with the merged branch, so the walk continues down
/// that parent's first-parent history: a fragment merged from `next` links the
/// pull request that added it there, not the merge. A merge titled with its
/// pull request, `Title (#12)`, whose branch commits name none is the commit
/// named instead, so a repository that merges with merge commits keeps its
/// pull request links: the deepest such merge on the way down wins.
///
/// No repository, and a file HEAD does not track, is `None`: a fixture outside
/// git, or a fragment not committed yet, has no commit to name. A shallow
/// clone that ends before the adding commit is `None` too: a commit listed in
/// `.git/shallow` is the boundary, so it counts as parentless, and naming it
/// would link the wrong commit. A parent that exists but cannot be loaded is
/// treated the same way.
pub fn added_by(root: &Path, path: &Path) -> Result<Option<AddedBy>> {
    let Ok(repo) = discover(root) else {
        return Ok(None);
    };
    let Some(rel) = git_path(root, path, repo.workdir()) else {
        return Ok(None);
    };
    let Ok(head) = repo.head_commit() else {
        return Ok(None);
    };
    if !has_path(&head, &rel)? {
        return Ok(None);
    }
    let boundary: Vec<gix::ObjectId> = repo
        .shallow_commits()?
        .map(|commits| commits.iter().copied().collect())
        .unwrap_or_default();
    let mut commit = head;
    let mut named_merge = None;
    loop {
        let parents: Vec<gix::ObjectId> = if boundary.contains(&commit.id().detach()) {
            Vec::new()
        } else {
            commit.parent_ids().map(gix::Id::detach).collect()
        };
        if parents.is_empty() && !boundary.is_empty() {
            return Ok(None);
        }
        let mut holder = None;
        for (index, id) in parents.into_iter().enumerate() {
            let Ok(parent) = repo.find_commit(id) else {
                return Ok(None);
            };
            if has_path(&parent, &rel)? {
                holder = Some((index, parent));
                break;
            }
        }
        let added = named(&commit)?;
        let Some((index, parent)) = holder else {
            return Ok(Some(match named_merge {
                Some(merge) if pull_request_number(&added.subject).is_none() => merge,
                Some(_) | None => added,
            }));
        };
        if index > 0 && pull_request_number(&added.subject).is_some() {
            named_merge = Some(added);
        }
        commit = parent;
    }
}

/// The commit that set a package's version, and how far it sits below HEAD.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct VersionCommit {
    /// Full hex SHA of the commit.
    pub sha: String,
    /// First-parent steps from HEAD: 0 is HEAD itself. A larger depth is an
    /// older commit, so sorting by depth, largest first, orders releases the
    /// way history made them.
    pub depth: usize,
}

/// The newest commit on HEAD's first-parent history where the version `read`
/// finds in `path` became the version it has at HEAD.
///
/// The walk follows first parents only, so a merge counts as the commit that
/// brought the version to this branch, which is where the release happens.
/// `read` returns `None` when a revision of the file holds no version, which
/// counts as a different version.
///
/// # Errors
///
/// Returns an error when `root` is in no repository, `path` is outside it,
/// HEAD has no version in `path`, a commit or tree cannot be read, or the walk
/// reaches a shallow clone's boundary before the version changes: the commit
/// that set it is not in the clone, so a full fetch is needed.
pub fn version_commit(
    root: &Path,
    path: &Path,
    read: &dyn Fn(&str) -> Option<String>,
) -> Result<VersionCommit> {
    let repo = discover(root).context("finding a version commit needs a git repository")?;
    let rel = git_path(root, path, repo.workdir())
        .with_context(|| format!("{} is not in the repository", path.display()))?;
    let version_at = |commit: &gix::Commit<'_>| -> Result<Option<String>> {
        Ok(blob_at(&commit.tree()?, &rel).and_then(|raw| read(&raw)))
    };
    let mut commit = repo
        .head_commit()
        .context("finding a version commit needs HEAD")?;
    let wanted = version_at(&commit)?
        .with_context(|| format!("HEAD has no version in {}", rel.display()))?;
    let boundary: Vec<gix::ObjectId> = repo
        .shallow_commits()?
        .map(|commits| commits.iter().copied().collect())
        .unwrap_or_default();
    let mut depth = 0;
    loop {
        ensure!(
            !boundary.contains(&commit.id().detach()),
            "cannot find the commit that set {} to {wanted}: the clone is shallow; fetch the full history",
            rel.display()
        );
        let Some(parent) = commit.parent_ids().next() else {
            break;
        };
        let parent = repo.find_commit(parent.detach())?;
        if version_at(&parent)?.as_deref() != Some(wanted.as_str()) {
            break;
        }
        commit = parent;
        depth += 1;
    }
    Ok(VersionCommit {
        sha: commit.id().to_string(),
        depth,
    })
}

fn has_path(commit: &gix::Commit<'_>, rel: &Path) -> Result<bool> {
    Ok(commit.tree()?.lookup_entry_by_path(rel)?.is_some())
}

fn named(commit: &gix::Commit<'_>) -> Result<AddedBy> {
    Ok(AddedBy {
        sha: commit.id().to_string(),
        subject: commit.message()?.summary().to_str_lossy().into_owned(),
    })
}

/// The pull request a squash-merge subject names: a trailing `(#12)`.
pub(crate) fn pull_request_number(subject: &str) -> Option<u64> {
    let (_, digits) = subject.trim_end().strip_suffix(')')?.rsplit_once("(#")?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// The branch `origin/HEAD` points at, or `None` when that ref is unset.
#[must_use]
pub fn upstream_default_branch(root: &Path) -> Option<String> {
    origin_head_branch(&discover(root).ok()?)
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

/// How to fetch `origin/<branch>` when that ref is missing.
pub enum FetchDefault {
    /// `git fetch origin`.
    Origin,
    /// GitHub HTTPS. Credentials are the callback, never the URL.
    GitHub {
        /// Access token for the fetch.
        token: String,
        /// The `owner/name` slug.
        repository: String,
    },
}

/// Prints the repository but never the token, so a logged value leaks no
/// credential.
impl std::fmt::Debug for FetchDefault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Origin => f.write_str("Origin"),
            Self::GitHub { repository, .. } => f
                .debug_struct("GitHub")
                .field("token", &"<redacted>")
                .field("repository", repository)
                .finish(),
        }
    }
}

impl FetchDefault {
    /// `GitHub` when `GITHUB_TOKEN` and `GITHUB_REPOSITORY` are both set and
    /// non-empty, else `Origin`.
    #[must_use]
    pub fn from_env() -> Self {
        match (env::var("GITHUB_TOKEN"), env::var("GITHUB_REPOSITORY")) {
            (Ok(token), Ok(repository)) if !token.is_empty() && !repository.is_empty() => {
                Self::GitHub { token, repository }
            }
            _ => Self::Origin,
        }
    }
}

/// HTTPS URL for an `owner/name` slug. No credentials in the URL.
#[must_use]
pub fn github_https_url(repository: &str) -> String {
    format!("https://github.com/{repository}.git")
}

/// Username GitHub git-over-HTTPS expects. Not an HTTP Bearer scheme.
#[must_use]
pub fn github_git_user() -> &'static str {
    "x-access-token"
}

/// `VERCTL_DEFAULT_BRANCH` only.
///
/// `GITHUB_BASE_REF` is a candidate for `require_on_default_history`,
/// not a name to rewrite `origin/HEAD` with.
#[must_use]
pub fn configured_default_branch() -> Option<String> {
    env::var("VERCTL_DEFAULT_BRANCH")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// Point `origin/HEAD` at `origin/<branch>`. Fetch that one ref only
/// when it is missing. The fetch is the branch's history, not a
/// depth-1 tip: `require_on_default_history` walks parents.
pub fn ensure_origin_head(root: &Path, branch: &str, fetch: &FetchDefault) -> Result<()> {
    if branch.is_empty() {
        return Ok(());
    }
    let Some(repo) = repo_covering(root) else {
        return Ok(());
    };
    if !has_origin(&repo) {
        return Ok(());
    }
    let remote_ref = format!("refs/remotes/origin/{branch}");
    if peel_remote(&repo, &remote_ref).is_none() {
        fetch_one_branch(&repo, branch, fetch)?;
    }
    ensure!(
        peel_remote(&repo, &remote_ref).is_some(),
        "cannot resolve the default branch ({remote_ref} is missing; fetch the default branch)"
    );
    let target = gix::refs::FullName::try_from(remote_ref.as_str())
        .context("point origin/HEAD at the default branch")?;
    let name = gix::refs::FullName::try_from("refs/remotes/origin/HEAD")
        .context("point origin/HEAD at the default branch")?;
    repo.edit_reference(RefEdit::update(
        name,
        Target::Symbolic(target),
        PreviousValue::Any,
        "verctl default branch",
    ))
    .context("point origin/HEAD at the default branch")?;
    Ok(())
}

/// Fetches `branch` alone into `origin/<branch>`, through a remote that carries
/// only that refspec and no tags, so the configured `origin` refspecs cannot
/// widen the fetch.
fn fetch_one_branch(repo: &gix::Repository, branch: &str, fetch: &FetchDefault) -> Result<()> {
    let spec = format!("+refs/heads/{branch}:refs/remotes/origin/{branch}");
    let url = match fetch {
        FetchDefault::Origin => origin_url_of(repo)?,
        FetchDefault::GitHub { repository, .. } => github_https_url(repository),
    };
    let remote = repo
        .remote_at(url.as_str())
        .context("fetch the default branch")?
        .with_fetch_tags(gix::remote::fetch::Tags::None)
        .with_refspecs([spec.as_str()], Direction::Fetch)
        .context("fetch the default branch")?;
    let connection = remote
        .connect(Direction::Fetch)
        .with_context(|| format!("git fetch {spec}"))?;
    let interrupt = AtomicBool::new(false);
    match fetch {
        FetchDefault::Origin => connection
            .prepare_fetch(
                gix::progress::Discard,
                gix::remote::ref_map::Options::default(),
            )
            .with_context(|| format!("git fetch {spec}"))?
            .receive(gix::progress::Discard, &interrupt),
        FetchDefault::GitHub { token, .. } => connection
            .with_credentials(github_credentials(token))
            .prepare_fetch(
                gix::progress::Discard,
                gix::remote::ref_map::Options::default(),
            )
            .with_context(|| format!("git fetch {spec}"))?
            .receive(gix::progress::Discard, &interrupt),
    }
    .with_context(|| format!("git fetch {spec}"))?;
    Ok(())
}

/// Answers every credential request with `x-access-token` and `token`, held in
/// memory: the token never reaches a URL, a config file, or a helper program.
fn github_credentials(token: &str) -> impl FnMut(Action) -> gix::Result<Option<Outcome>> + '_ {
    move |action| match action {
        Action::Get(context) => Ok(Some(Outcome {
            identity: Account {
                username: github_git_user().to_owned(),
                password: token.to_owned(),
                oauth_refresh_token: None,
            },
            next: context.into(),
        })),
        Action::Store(_) | Action::Erase(_) => Ok(None),
    }
}

fn prove_default_history(root: &Path, candidates: &[String]) -> Result<()> {
    let Some(repo) = repo_covering(root) else {
        return Ok(());
    };
    let Ok(head) = repo.head_commit() else {
        return Ok(());
    };
    let head = head.id().detach();
    let Some(upstream) = default_upstream_commit(&repo, candidates)? else {
        return Ok(());
    };
    if head == upstream {
        return Ok(());
    }
    match repo.merge_base(head, upstream) {
        Ok(Some(base)) if base == head => Ok(()),
        Ok(Some(_) | None) => bail!(
            "publish only from a Version PR commit on the default branch (HEAD {head} is not an ancestor of {upstream})"
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
    let index = repo.index_or_empty().context("git index")?;
    let project = project_prefix(&repo, root);
    Ok(index
        .entries()
        .iter()
        .filter_map(|entry| entry.path(&index).to_str().ok().map(PathBuf::from))
        .filter_map(|path| path.strip_prefix(&project).ok().map(Path::to_path_buf))
        .collect())
}

/// Whether the repository is told to ignore `rel`, a path relative to `root`.
///
/// Ignoring a file is a repo saying it is not part of what it publishes — the
/// same statement `tracked` already reads the index for. So an ignored file is
/// not a forgotten one, and nothing should ask its author to commit it. A file
/// under an ignored directory is ignored too, as git has it: gix matches the
/// one path it is given, so each parent directory is asked first.
#[must_use]
pub fn is_ignored(root: &Path, rel: &Path) -> bool {
    let Some(repo) = repo_covering(root) else {
        return false;
    };
    let path = project_prefix(&repo, root).join(rel);
    let Ok(index) = repo.index_or_empty() else {
        return false;
    };
    let Ok(mut excludes) = repo.excludes(
        &index,
        None,
        gix::worktree::stack::state::ignore::Source::WorktreeThenIdMappingIfNotSkipped,
    ) else {
        return false;
    };
    let mut ignored_at = |path: &Path, mode| {
        excludes
            .at_path(path, mode)
            .is_ok_and(|platform| platform.is_excluded())
    };
    path.ancestors()
        .skip(1)
        .filter(|dir| !dir.as_os_str().is_empty())
        .any(|dir| ignored_at(dir, Some(gix::index::entry::Mode::DIR)))
        || ignored_at(&path, None)
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
fn project_prefix(repo: &gix::Repository, root: &Path) -> PathBuf {
    let Some(workdir) = repo.workdir().and_then(|dir| dir.canonicalize().ok()) else {
        return PathBuf::new();
    };
    root.canonicalize()
        .ok()
        .and_then(|root| root.strip_prefix(&workdir).ok().map(Path::to_path_buf))
        .unwrap_or_default()
}

/// The repository that covers `dir`, searching upward, as it is on disk.
///
/// A git hook exports repository-local variables for its own git process:
/// pre-commit sets `GIT_INDEX_FILE` to a temporary index, given as a path
/// relative to git's working directory, and may set `GIT_DIR` and
/// `GIT_WORK_TREE`. verctl ignores them, as libgit2 did, so a command answers
/// the same inside a hook and outside one: gix's catch-all permission for
/// `GIT_*` variables is denied, which drops those overrides, while identity
/// and HTTP variables keep their own permissions. `GIT_DIR` is never read,
/// because discovery starts from `dir`, not from the environment. Nothing in
/// this module writes the index.
///
/// Fetches and branch updates write reflog entries, which gix refuses without
/// a committer, and a CI runner often has no git identity configured: the
/// [`fallback_identity`] fills that gap, while a configured identity still
/// wins.
fn discover(dir: &Path) -> Result<gix::Repository> {
    let mapping = Mapping {
        full: open_options(Trust::Full),
        reduced: open_options(Trust::Reduced),
    };
    let repo = gix::ThreadSafeRepository::discover_opts(
        dir,
        gix::discover::upwards::Options::default(),
        mapping,
    )
    .with_context(|| format!("no git repository covers {}", dir.display()))?;
    let mut repo = repo.to_thread_local();
    let (name, email) = fallback_identity();
    repo.committer_or_set_fallback(name.as_str(), email.as_str())
        .context("set the reflog identity")?;
    Ok(repo)
}

/// gix's defaults for `trust`, without the repository-local `GIT_*` overrides.
fn open_options(trust: Trust) -> gix::open::Options {
    let mut options = gix::open::Options::default_for_level(trust);
    options.permissions.env.git_prefix = Permission::Deny;
    options
}

fn repo_covering(root: &Path) -> Option<gix::Repository> {
    let repo = discover(root).ok()?;
    let workdir = repo.workdir()?.canonicalize().ok()?;
    let root = root.canonicalize().ok()?;
    root.starts_with(workdir).then_some(repo)
}

fn has_origin(repo: &gix::Repository) -> bool {
    repo.remote_names()
        .iter()
        .any(|name| name.to_str() == Ok("origin"))
}

fn origin_url_of(repo: &gix::Repository) -> Result<String> {
    let remote = repo
        .find_remote("origin")
        .context("fetch the default branch")?;
    let url = remote.url(Direction::Fetch).context("origin has no URL")?;
    Ok(url.to_bstring().to_str_lossy().into_owned())
}

fn peel_remote(repo: &gix::Repository, name: &str) -> Option<gix::ObjectId> {
    let mut reference = repo.try_find_reference(name).ok()??;
    let commit = reference.peel_to_commit().ok()?;
    Some(commit.id().detach())
}

fn origin_head_branch(repo: &gix::Repository) -> Option<String> {
    let reference = repo.try_find_reference("refs/remotes/origin/HEAD").ok()??;
    let target = reference.target();
    let target = target.try_name()?.as_bstr().to_str().ok()?;
    let name = target.trim_start_matches("refs/remotes/origin/");
    (!name.is_empty()).then(|| name.to_owned())
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
/// `ensure_origin_head` fetches and points at
/// `VERCTL_DEFAULT_BRANCH`.
#[must_use]
pub fn default_branch_candidates() -> Vec<String> {
    candidate_names(env_base_ref().as_deref())
}

/// Like `default_branch_candidates`, with the base ref passed in instead of
/// read from the environment.
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

fn default_upstream_commit(
    repo: &gix::Repository,
    candidates: &[String],
) -> Result<Option<gix::ObjectId>> {
    if let Some(branch) = origin_head_branch(repo) {
        let name = format!("refs/remotes/origin/{branch}");
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
    if has_origin(repo) {
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
/// the unexpected-dirty check; not a filesystem glob. Renames are not
/// tracked, so a moved file shows as its old and its new path, each of
/// which must be allowed.
pub fn assert_only_allowed(
    root: &Path,
    allowed: &[PathBuf],
    globs: &[String],
    stage_ignored: bool,
) -> Result<Vec<PathBuf>> {
    let Ok(repo) = discover(root) else {
        return Ok(Vec::new());
    };
    let dirty = status_paths(&repo, Vec::new(), false).context("git status")?;
    let workdir = repo.workdir().unwrap_or(root);
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let workdir = workdir
        .canonicalize()
        .unwrap_or_else(|_| workdir.to_path_buf());
    let mut extra = Vec::new();
    let mut staged = Vec::new();
    for rel in &dirty {
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
        extra.push(rel.clone());
    }
    if stage_ignored && !globs.is_empty() {
        let patterns = globs
            .iter()
            .map(|pattern| pattern.as_str().into())
            .collect();
        let ignored = status_paths(&repo, patterns, true).context("git status (ignored)")?;
        for rel in &ignored {
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

/// The paths `git status` reports, one entry per path, sorted.
///
/// With `ignored` unset: staged, modified, deleted, and untracked files, every
/// untracked file listed on its own. With `ignored` set: only the ignored
/// files `patterns` select, each listed on its own.
fn status_paths(
    repo: &gix::Repository,
    patterns: Vec<gix::bstr::BString>,
    ignored: bool,
) -> Result<BTreeSet<String>> {
    let status = repo
        .status(gix::progress::Discard)?
        .untracked_files(gix::status::UntrackedFiles::Files)
        .index_worktree_rewrites(None)
        .tree_index_track_renames(gix::status::tree_index::TrackRenames::Disabled);
    let status = if ignored {
        status.dirwalk_options(|options| {
            options.emit_ignored(Some(gix::dir::walk::EmissionMode::Matching))
        })
    } else {
        status
    };
    let mut paths = BTreeSet::new();
    for item in status.into_iter(patterns)? {
        let item = item?;
        let is_ignored = matches!(
            &item,
            gix::status::Item::IndexWorktree(gix::status::index_worktree::Item::DirectoryContents {
                entry,
                ..
            }) if matches!(entry.status, gix::dir::entry::Status::Ignored(_))
        );
        if is_ignored == ignored {
            paths.insert(item.location().to_str_lossy().into_owned());
        }
    }
    Ok(paths)
}

/// What [`commit_branch_and_push`] did: `Empty` when the tree was unchanged.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PushOutcome {
    /// No version files changed.
    Empty,
    /// Branch was updated and pushed.
    Pushed,
}

/// Commits `paths` onto `branch` without moving HEAD, then force-pushes
/// `branch` to `github` over HTTPS with `token`.
pub fn commit_branch_and_push(
    root: &Path,
    branch: &str,
    message: &str,
    token: &str,
    github: &Repo,
    paths: &[PathBuf],
) -> Result<PushOutcome> {
    let repo = discover(root).context("open git repository")?;
    let workdir = absolute_workdir(&repo)?;
    let parent = repo.head_commit().context("HEAD commit")?;
    let Some(_) = write_commit_on_branch(&repo, &workdir, &parent, branch, message, paths)? else {
        return Ok(PushOutcome::Empty);
    };
    let url = format!("https://github.com/{}/{}.git", github.owner, github.name);
    push(repo.git_dir(), &url, branch, token)?;
    Ok(PushOutcome::Pushed)
}

/// The working tree as an absolute path. gix reports it as it was discovered,
/// so a repository found from `.` has `.` for a workdir, and no path would
/// strip that prefix.
fn absolute_workdir(repo: &gix::Repository) -> Result<PathBuf> {
    let workdir = repo.workdir().context("repository has no workdir")?;
    std::path::absolute(workdir)
        .with_context(|| format!("resolve the workdir {}", workdir.display()))
}

/// The token reaches `git push` through this variable, which only the push's
/// own process sees.
const PUSH_TOKEN: &str = "VERCTL_GIT_TOKEN";

/// The credential helper `git push` runs: it answers `get` with
/// `x-access-token` and the token from [`PUSH_TOKEN`], and ignores `store`
/// and `erase`, so the token is never in argv, a URL, or a credential store.
const PUSH_CREDENTIAL_HELPER: &str = r#"!f() { test "$1" = get || return 0; echo username=x-access-token; echo "password=$VERCTL_GIT_TOKEN"; }; f"#;

/// How long a push may run before it is killed.
const PUSH_TIMEOUT: Duration = Duration::from_mins(5);

/// Force-pushes `branch` to `url` with the `git` binary, because gix cannot
/// push yet.
///
/// `credential.helper=` first clears every helper the user configured, so no
/// keychain answers or stores in its place; `GIT_TERMINAL_PROMPT=0` makes a
/// rejected token fail instead of waiting for input. The repository's own
/// hooks do not run (`--no-verify`): the branch is generated, and libgit2,
/// which pushed it before, never ran hooks either. `--git-dir` names the
/// repository outright, so a `GIT_DIR` a hook exported cannot redirect the
/// push. The error, which carries git's stderr, can hold the URL but not the
/// token, since git only ever saw the variable's name.
fn push(git_dir: &Path, url: &str, branch: &str, token: &str) -> Result<()> {
    let git_dir = git_dir
        .canonicalize()
        .with_context(|| format!("git directory {}", git_dir.display()))?;
    let git_dir = format!(
        "--git-dir={}",
        git_dir.to_str().context("git directory is not UTF-8")?
    );
    let helper = format!("credential.helper={PUSH_CREDENTIAL_HELPER}");
    let refspec = format!("+refs/heads/{branch}:refs/heads/{branch}");
    let argv = process::argv(&[
        "git",
        &git_dir,
        "-c",
        "credential.helper=",
        "-c",
        &helper,
        "push",
        "--no-verify",
        "--quiet",
        url,
        &refspec,
    ]);
    process::run_limited(
        &argv,
        &[(PUSH_TOKEN, token), ("GIT_TERMINAL_PROMPT", "0")],
        PUSH_TIMEOUT,
    )
    .context("git push")?;
    Ok(())
}

/// Writes a commit of `parent`'s tree with `paths` taken from the working
/// tree, and points `branch` at it. HEAD, the index, and the working tree are
/// left alone: the tree is edited in memory. `None` when the tree is
/// unchanged. A path that no longer exists is removed from the tree. Each file
/// passes through the `.gitattributes` clean filters first, as `git add` runs
/// them, so `text` normalization and LFS pointers match what git would store.
fn write_commit_on_branch(
    repo: &gix::Repository,
    workdir: &Path,
    parent: &gix::Commit<'_>,
    branch: &str,
    message: &str,
    paths: &[PathBuf],
) -> Result<Option<gix::ObjectId>> {
    let parent_tree = parent.tree_id().context("HEAD tree")?.detach();
    let mut editor = repo.edit_tree(parent_tree).context("edit HEAD tree")?;
    let (mut filters, index) = repo
        .filter_pipeline(None)
        .context("load the .gitattributes filters")?;
    for path in paths {
        let rel = workdir_rel(workdir, path)?;
        let name = rel
            .to_str()
            .with_context(|| format!("path is not UTF-8: {}", rel.display()))?
            .to_owned();
        let abs = workdir.join(&rel);
        if abs.exists() {
            let (kind, data) = worktree_entry(&mut filters, &index, &abs, &rel)?;
            let blob = repo
                .write_blob(data)
                .with_context(|| format!("git add {name}"))?;
            editor
                .upsert(&name, kind, blob)
                .with_context(|| format!("git add {name}"))?;
        } else {
            editor
                .remove(&name)
                .with_context(|| format!("git rm {name}"))?;
        }
    }
    let tree = editor.write().context("write tree")?.detach();
    if tree == parent_tree {
        return Ok(None);
    }
    let signature = signature(repo);
    let commit = gix::objs::Commit {
        tree,
        parents: [parent.id().detach()].into_iter().collect(),
        author: signature.clone(),
        committer: signature,
        encoding: None,
        message: message.into(),
        extra_headers: Vec::new(),
    };
    let id = repo.write_object(&commit).context("git commit")?.detach();
    repo.reference(
        format!("refs/heads/{branch}"),
        id,
        PreviousValue::Any,
        "verctl: version packages",
    )
    .context("git commit")?;
    Ok(Some(id))
}

/// The tree entry kind and content for the file at `abs`, which sits at `rel`
/// in the working tree: a symbolic link stores its target, and a file stores
/// what the clean `filters` make of it and keeps its executable bit.
fn worktree_entry(
    filters: &mut gix::filter::Pipeline<'_>,
    index: &gix::index::State,
    abs: &Path,
    rel: &Path,
) -> Result<(EntryKind, Vec<u8>)> {
    let meta = std::fs::symlink_metadata(abs).with_context(|| abs.display().to_string())?;
    if meta.file_type().is_symlink() {
        let target = std::fs::read_link(abs).with_context(|| abs.display().to_string())?;
        let target = target
            .to_str()
            .with_context(|| format!("link target is not UTF-8: {}", abs.display()))?;
        return Ok((EntryKind::Link, target.as_bytes().to_vec()));
    }
    let file = std::fs::File::open(abs).with_context(|| abs.display().to_string())?;
    let mut clean = filters
        .convert_to_git(file, rel, index)
        .with_context(|| format!("filter {}", rel.display()))?;
    let mut data = Vec::new();
    std::io::Read::read_to_end(&mut clean, &mut data)
        .with_context(|| format!("filter {}", rel.display()))?;
    Ok((file_kind(&meta), data))
}

#[cfg(unix)]
fn file_kind(meta: &std::fs::Metadata) -> EntryKind {
    use std::os::unix::fs::PermissionsExt;
    if meta.permissions().mode() & 0o111 == 0 {
        EntryKind::Blob
    } else {
        EntryKind::BlobExecutable
    }
}

#[cfg(not(unix))]
fn file_kind(_meta: &std::fs::Metadata) -> EntryKind {
    EntryKind::Blob
}

/// `path` relative to the working tree. A relative `path` is taken from the
/// workdir. An absolute one may spell the workdir as given or canonical:
/// the dirty-path check returns canonical paths, while the plan keeps the
/// spelling verctl was started with, and a checkout behind a symlink spells
/// them differently. Both spellings of the workdir are tried against `path`
/// as given and against `path` with its parent canonicalized, which still
/// works for a file that was deleted.
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
    let workdirs = [Some(workdir.to_path_buf()), workdir.canonicalize().ok()];
    let through_parent = normalized
        .parent()
        .and_then(|parent| parent.canonicalize().ok())
        .zip(normalized.file_name())
        .map(|(parent, name)| parent.join(name));
    let paths = [Some(normalized.clone()), through_parent];
    paths
        .iter()
        .flatten()
        .find_map(|path| {
            workdirs
                .iter()
                .flatten()
                .find_map(|workdir| path.strip_prefix(workdir).ok())
        })
        .map(Path::to_path_buf)
        .with_context(|| format!("path outside workdir: {}", normalized.display()))
}

/// Who the Version PR commit is by: `user.name` and `user.email` when the
/// repository's git config sets both, else the [`fallback_identity`].
fn signature(repo: &gix::Repository) -> gix::actor::Signature {
    let time = gix::date::Time::now_local_or_utc();
    let config = repo.config_snapshot();
    if let (Some(name), Some(email)) = (config.string("user.name"), config.string("user.email")) {
        return gix::actor::Signature { name, email, time };
    }
    let (name, email) = fallback_identity();
    gix::actor::Signature {
        name: name.into(),
        email: email.into(),
        time,
    }
}

/// The identity verctl writes under when git config names none:
/// `GIT_AUTHOR_NAME` or `GITHUB_ACTOR`, else `verctl`, with
/// `GIT_AUTHOR_EMAIL` or that name's GitHub noreply address.
fn fallback_identity() -> (String, String) {
    let name = env::var("GIT_AUTHOR_NAME")
        .or_else(|_| env::var("GITHUB_ACTOR"))
        .unwrap_or_else(|_| "verctl".into());
    let email =
        env::var("GIT_AUTHOR_EMAIL").unwrap_or_else(|_| format!("{name}@users.noreply.github.com"));
    (name, email)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use indoc::indoc;

    use super::{AddedBy, VersionCommit, added_by, origin_url, version_commit};
    use crate::test_git::{commit_all, git, init};

    /// The paths of every blob in the commit `sha`.
    fn tree_names(dir: &Path, sha: &str) -> Vec<String> {
        git(dir, &["ls-tree", "-r", "--name-only", sha])
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn version_line(raw: &str) -> Option<String> {
        raw.lines()
            .find_map(|line| line.strip_prefix("version = "))
            .map(|version| version.trim_matches('"').to_owned())
    }

    fn write_version(dir: &Path, version: &str) {
        std::fs::write(
            dir.join("Cargo.toml"),
            indoc::formatdoc! {r#"
                [package]
                name = "demo"
                version = "{version}"
            "#},
        )
        .unwrap();
    }

    #[test]
    fn head_moved_past_the_release_finds_the_commit_that_set_it() {
        let dir = tempfile::TempDir::new().unwrap();
        init(dir.path());
        write_version(dir.path(), "0.4.5");
        commit_all(dir.path(), "chore: start");
        write_version(dir.path(), "0.4.6");
        let set = commit_all(dir.path(), "chore(release): version packages (#7)");
        std::fs::write(dir.path().join("README"), "later\n").unwrap();
        commit_all(dir.path(), "docs: later (#8)");
        let found =
            version_commit(dir.path(), &dir.path().join("Cargo.toml"), &version_line).unwrap();
        assert_eq!(found, VersionCommit { sha: set, depth: 1 });
    }

    #[test]
    fn the_first_commit_is_the_version_commit_when_nothing_changed_it() {
        let dir = tempfile::TempDir::new().unwrap();
        init(dir.path());
        write_version(dir.path(), "0.0.1");
        let first = commit_all(dir.path(), "chore: start");
        let found =
            version_commit(dir.path(), &dir.path().join("Cargo.toml"), &version_line).unwrap();
        assert_eq!(found.sha, first);
        assert_eq!(found.depth, 0);
    }

    #[test]
    fn a_version_set_again_after_a_revert_names_the_newest_change() {
        let dir = tempfile::TempDir::new().unwrap();
        init(dir.path());
        write_version(dir.path(), "0.4.6");
        commit_all(dir.path(), "chore: start");
        write_version(dir.path(), "0.4.5");
        commit_all(dir.path(), "revert: 0.4.6");
        write_version(dir.path(), "0.4.6");
        let again = commit_all(dir.path(), "chore(release): version packages (#9)");
        let found =
            version_commit(dir.path(), &dir.path().join("Cargo.toml"), &version_line).unwrap();
        assert_eq!(found.sha, again);
    }

    #[test]
    fn added_by_names_the_commit_that_added_the_file() {
        let dir = tempfile::TempDir::new().unwrap();
        init(dir.path());
        std::fs::write(dir.path().join("README"), "start\n").unwrap();
        commit_all(dir.path(), "chore: start");
        std::fs::write(dir.path().join("fix.md"), "Fix.\n").unwrap();
        let added = commit_all(dir.path(), "fix: the parser (#12)");
        std::fs::write(dir.path().join("README"), "later\n").unwrap();
        commit_all(dir.path(), "docs: later");
        assert_eq!(
            added_by(dir.path(), &dir.path().join("fix.md")).unwrap(),
            Some(AddedBy {
                sha: added,
                subject: "fix: the parser (#12)".into(),
            })
        );
    }

    /// A fragment that reached HEAD through a merge commit names the commit
    /// that added it on the merged branch, not the merge.
    #[test]
    fn added_by_follows_a_merge_to_the_commit_that_added_the_file() {
        let dir = tempfile::TempDir::new().unwrap();
        init(dir.path());
        std::fs::write(dir.path().join("README"), "start\n").unwrap();
        let base = commit_all(dir.path(), "chore: start");
        std::fs::write(dir.path().join("fix.md"), "Fix.\n").unwrap();
        let side = commit_all(dir.path(), "fix: the parser (#12)");
        git(dir.path(), &["reset", "-q", "--hard", &base]);
        std::fs::write(dir.path().join("README"), "later\n").unwrap();
        let main = commit_all(dir.path(), "docs: later (#13)");
        std::fs::write(dir.path().join("fix.md"), "Fix.\n").unwrap();
        git(dir.path(), &["add", "-A"]);
        let tree = git(dir.path(), &["write-tree"]);
        let merge = git(
            dir.path(),
            &[
                "commit-tree",
                &tree,
                "-p",
                &main,
                "-p",
                &side,
                "-m",
                "feat: release next (#14)",
            ],
        );
        git(dir.path(), &["reset", "-q", "--hard", &merge]);
        assert_eq!(
            added_by(dir.path(), &dir.path().join("fix.md")).unwrap(),
            Some(AddedBy {
                sha: side,
                subject: "fix: the parser (#12)".into(),
            })
        );
    }

    /// A merge titled with its pull request is named when the branch commit
    /// that added the file names none: merge-commit repositories keep their
    /// pull request links.
    #[test]
    fn added_by_names_a_titled_merge_over_an_unnamed_branch_commit() {
        let dir = tempfile::TempDir::new().unwrap();
        init(dir.path());
        std::fs::write(dir.path().join("README"), "start\n").unwrap();
        let base = commit_all(dir.path(), "chore: start");
        std::fs::write(dir.path().join("fix.md"), "Fix.\n").unwrap();
        let side = commit_all(dir.path(), "wip");
        git(dir.path(), &["reset", "-q", "--hard", &base]);
        std::fs::write(dir.path().join("fix.md"), "Fix.\n").unwrap();
        git(dir.path(), &["add", "-A"]);
        let tree = git(dir.path(), &["write-tree"]);
        let merge = git(
            dir.path(),
            &[
                "commit-tree",
                &tree,
                "-p",
                &base,
                "-p",
                &side,
                "-m",
                "fix: the parser (#12)",
            ],
        );
        git(dir.path(), &["reset", "-q", "--hard", &merge]);
        assert_eq!(
            added_by(dir.path(), &dir.path().join("fix.md")).unwrap(),
            Some(AddedBy {
                sha: merge,
                subject: "fix: the parser (#12)".into(),
            })
        );
    }

    /// A shallow clone ends at a commit git reports as parentless. When the
    /// adding commit lies beyond it, naming the boundary would be wrong.
    #[test]
    fn added_by_names_nothing_past_a_shallow_boundary() {
        let dir = tempfile::TempDir::new().unwrap();
        init(dir.path());
        std::fs::write(dir.path().join("fix.md"), "Fix.\n").unwrap();
        commit_all(dir.path(), "fix: the parser (#12)");
        std::fs::write(dir.path().join("README"), "later\n").unwrap();
        let boundary = commit_all(dir.path(), "docs: later (#13)");
        std::fs::write(dir.path().join(".git/shallow"), format!("{boundary}\n")).unwrap();
        assert_eq!(
            git(dir.path(), &["rev-parse", "--is-shallow-repository"]),
            "true"
        );
        assert_eq!(
            added_by(dir.path(), &dir.path().join("fix.md")).unwrap(),
            None
        );
    }

    #[test]
    fn origin_url_from_discovered_repo() {
        let dir = tempfile::TempDir::new().unwrap();
        init(dir.path());
        git(
            dir.path(),
            &[
                "remote",
                "add",
                "origin",
                "https://github.com/victor-software-house/verctl.git",
            ],
        );
        commit_all(dir.path(), "init");
        assert_eq!(
            origin_url(dir.path()).unwrap(),
            "https://github.com/victor-software-house/verctl.git"
        );
    }

    #[test]
    fn empty_paths_do_not_switch_head() {
        let dir = tempfile::TempDir::new().unwrap();
        init(dir.path());
        commit_all(dir.path(), "init");
        let before = git(dir.path(), &["symbolic-ref", "HEAD"]);
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
        assert_eq!(git(dir.path(), &["symbolic-ref", "HEAD"]), before);
    }

    #[test]
    fn unexpected_dirty_fails_unless_globbed() {
        let dir = tempfile::TempDir::new().unwrap();
        init(dir.path());
        std::fs::write(
            dir.path().join("Cargo.toml"),
            indoc! {r#"
                [package]
                version = "1.0.0"
            "#},
        )
        .unwrap();
        commit_all(dir.path(), "init");
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
        init(dir.path());
        std::fs::write(dir.path().join("Cargo.lock"), "old\n").unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            indoc! {r#"
                [package]
                version = "1.0.0"
            "#},
        )
        .unwrap();
        commit_all(dir.path(), "init");
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
        init(dir.path());
        std::fs::write(
            dir.path().join("Cargo.toml"),
            indoc! {r#"
                [package]
                version = "1.0.0"
            "#},
        )
        .unwrap();
        commit_all(dir.path(), "init");
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
        init(dir.path());
        std::fs::write(dir.path().join(".gitignore"), "/target\n").unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            indoc! {r#"
                [package]
                version = "1.0.0"
            "#},
        )
        .unwrap();
        commit_all(dir.path(), "init");
        std::fs::create_dir_all(dir.path().join("target/debug")).unwrap();
        std::fs::write(dir.path().join("target/debug/verctl"), "bin\n").unwrap();
        super::assert_only_allowed(dir.path(), &[dir.path().join("Cargo.toml")], &[], false)
            .unwrap();
    }

    #[test]
    fn stage_ignored_opt_in_collects_gitignored_files() {
        let dir = tempfile::TempDir::new().unwrap();
        init(dir.path());
        std::fs::write(dir.path().join(".gitignore"), "/generated.rs\n").unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            indoc! {r#"
                [package]
                version = "1.0.0"
            "#},
        )
        .unwrap();
        commit_all(dir.path(), "init");
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
        init(dir.path());
        std::fs::write(dir.path().join(".gitignore"), "/dist\n").unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            indoc! {r#"
                [package]
                version = "1.0.0"
            "#},
        )
        .unwrap();
        commit_all(dir.path(), "init");
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
        init(dir.path());
        std::fs::write(dir.path().join(".gitignore"), "/dist\n/target\n").unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            indoc! {r#"
                [package]
                version = "1.0.0"
            "#},
        )
        .unwrap();
        commit_all(dir.path(), "init");
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
        init(dir.path());
        std::fs::write(dir.path().join("Cargo.lock"), "old\n").unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            indoc! {r#"
                [package]
                version = "1.0.0"
            "#},
        )
        .unwrap();
        commit_all(dir.path(), "init");
        std::fs::remove_file(dir.path().join("Cargo.lock")).unwrap();
        let staged = super::assert_only_allowed(
            dir.path(),
            &[dir.path().join("Cargo.toml")],
            &["Cargo.lock".into()],
            false,
        )
        .unwrap();
        let oid = write_commit(dir.path(), &staged).expect("deletion should commit");
        let names = tree_names(dir.path(), &oid);
        assert!(!names.iter().any(|name| name == "Cargo.lock"), "{names:?}");
        assert!(names.iter().any(|name| name == "Cargo.toml"), "{names:?}");
    }

    #[test]
    fn commit_only_lists_given_paths() {
        let dir = tempfile::TempDir::new().unwrap();
        init(dir.path());
        std::fs::write(
            dir.path().join("Cargo.toml"),
            indoc! {r#"
                [package]
                version = "1.0.0"
            "#},
        )
        .unwrap();
        let parent = commit_all(dir.path(), "init");
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
        let oid = write_commit(dir.path(), &[dir.path().join("Cargo.toml")]).expect("commit");
        let names = tree_names(dir.path(), &oid);
        assert!(names.iter().any(|name| name == "Cargo.toml"), "{names:?}");
        assert!(!names.iter().any(|name| name == "secret.env"), "{names:?}");
        assert_eq!(git(dir.path(), &["rev-parse", "HEAD"]), parent);
    }

    #[test]
    fn dotted_relative_paths_stage() {
        let dir = tempfile::TempDir::new().unwrap();
        init(dir.path());
        std::fs::write(
            dir.path().join("Cargo.toml"),
            indoc! {r#"
                [package]
                version = "1.0.0"
            "#},
        )
        .unwrap();
        commit_all(dir.path(), "init");
        std::fs::write(
            dir.path().join("Cargo.toml"),
            indoc! {r#"
                [package]
                version = "1.0.1"
            "#},
        )
        .unwrap();
        let oid =
            write_commit(dir.path(), &[Path::new("./Cargo.toml").to_path_buf()]).expect("commit");
        let names = tree_names(dir.path(), &oid);
        assert!(names.iter().any(|name| name == "Cargo.toml"), "{names:?}");
    }

    /// The push's credential helper hands the server `x-access-token` and the
    /// token, and a rejected push reports an error without the token.
    #[tokio::test]
    async fn push_keeps_the_token_out_of_the_error() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::any())
            .respond_with(
                wiremock::ResponseTemplate::new(401)
                    .insert_header("WWW-Authenticate", r#"Basic realm="git""#),
            )
            .mount(&server)
            .await;
        let (dir, _) = fixture();
        git(dir.path(), &["branch", "version-packages"]);
        let url = format!("{}/owner/repo.git", server.uri());
        let token = "verctl-push-token-5f3a9c";
        let error = tokio::task::spawn_blocking(move || {
            super::push(&dir.path().join(".git"), &url, "version-packages", token)
        })
        .await
        .unwrap()
        .unwrap_err();
        let message = format!("{error:#}");
        assert!(!message.contains(token), "{message}");
        let expected = format!("Basic {}", base64(&format!("x-access-token:{token}")));
        let requests = server.received_requests().await.unwrap();
        assert!(
            requests.iter().any(|request| {
                request
                    .headers
                    .get("authorization")
                    .is_some_and(|value| value.as_bytes() == expected.as_bytes())
            }),
            "no request carried the token as Basic credentials"
        );
    }

    /// Standard base64 with padding, enough to spell an HTTP Basic header.
    fn base64(text: &str) -> String {
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in text.as_bytes().chunks(3) {
            let bytes = [
                chunk[0],
                chunk.get(1).copied().unwrap_or(0),
                chunk.get(2).copied().unwrap_or(0),
            ];
            let joined = u32::from(bytes[0]) << 16 | u32::from(bytes[1]) << 8 | u32::from(bytes[2]);
            for index in 0..4 {
                if index <= chunk.len() {
                    let sextet = (joined >> (18 - 6 * index)) & 0x3f;
                    out.push(char::from(ALPHABET[usize::try_from(sextet).unwrap()]));
                } else {
                    out.push('=');
                }
            }
        }
        out
    }

    /// A file under a `text` attribute is stored with LF endings, as `git add`
    /// stores it, whatever the working tree holds.
    #[test]
    fn the_commit_applies_clean_filters() {
        let dir = tempfile::TempDir::new().unwrap();
        init(dir.path());
        std::fs::write(dir.path().join(".gitattributes"), "*.md text\n").unwrap();
        std::fs::write(dir.path().join("CHANGELOG.md"), "old\n").unwrap();
        commit_all(dir.path(), "init");
        std::fs::write(dir.path().join("CHANGELOG.md"), "a\r\nb\r\n").unwrap();
        let oid = write_commit(dir.path(), &[dir.path().join("CHANGELOG.md")]).expect("commit");
        let blob = format!("{oid}:CHANGELOG.md");
        assert_eq!(git(dir.path(), &["cat-file", "-p", &blob]), "a\nb");
    }

    /// A configured `user.name` and `user.email` sign the Version PR commit
    /// and its reflog entry; the fallback identity fills in only when git
    /// config names none.
    #[test]
    fn a_configured_identity_wins_over_the_fallback() {
        let dir = tempfile::TempDir::new().unwrap();
        init(dir.path());
        git(dir.path(), &["config", "user.name", "Configured Name"]);
        git(
            dir.path(),
            &["config", "user.email", "configured@example.com"],
        );
        std::fs::write(dir.path().join("Cargo.toml"), "version = 1\n").unwrap();
        commit_all(dir.path(), "init");
        std::fs::write(dir.path().join("Cargo.toml"), "version = 2\n").unwrap();
        let oid = write_commit(dir.path(), &[dir.path().join("Cargo.toml")]).expect("commit");
        let configured = "Configured Name <configured@example.com>";
        assert_eq!(
            git(
                dir.path(),
                &["log", "-1", "--format=%an <%ae>|%cn <%ce>", &oid]
            ),
            format!("{configured}|{configured}")
        );
        assert_eq!(
            git(
                dir.path(),
                &[
                    "log",
                    "-g",
                    "-1",
                    "--format=%gn <%ge>",
                    "refs/heads/version-packages",
                ],
            ),
            configured
        );
    }

    /// `prepare --pr` runs from the repository root and finds it as `.`, and
    /// gix then reports `.` as the workdir, which no path strips. The commit
    /// resolves the workdir absolute first. The test binary runs in this
    /// crate's checkout, which is that root.
    #[test]
    fn a_workdir_found_from_dot_resolves_paths() {
        let repo = super::discover(Path::new(".")).unwrap();
        let workdir = super::absolute_workdir(&repo).unwrap();
        assert!(workdir.is_absolute(), "{}", workdir.display());
        assert_eq!(
            super::workdir_rel(&workdir, Path::new("./Cargo.toml")).unwrap(),
            Path::new("Cargo.toml")
        );
    }

    /// A checkout reached through a symlink has a workdir spelled through the
    /// link, while the dirty-path check returns canonical paths; both resolve.
    #[cfg(unix)]
    #[test]
    fn a_symlinked_workdir_takes_canonical_paths() {
        let dir = tempfile::TempDir::new().unwrap();
        init(dir.path());
        std::fs::write(dir.path().join("Cargo.toml"), "version = 1\n").unwrap();
        commit_all(dir.path(), "init");
        std::fs::write(dir.path().join("Cargo.toml"), "version = 2\n").unwrap();
        let links = tempfile::TempDir::new().unwrap();
        let link = links.path().join("checkout");
        std::os::unix::fs::symlink(dir.path(), &link).unwrap();
        let canonical = dir.path().canonicalize().unwrap().join("Cargo.toml");
        let workdir = super::absolute_workdir(&super::discover(&link).unwrap()).unwrap();
        assert_eq!(
            super::workdir_rel(&workdir, &canonical).unwrap(),
            Path::new("Cargo.toml")
        );
        assert_eq!(
            super::workdir_rel(&workdir, &link.join("Cargo.toml")).unwrap(),
            Path::new("Cargo.toml")
        );
    }

    /// Writes the Version PR commit of `paths` the way `commit_branch_and_push`
    /// does, and returns its SHA.
    fn write_commit(dir: &Path, paths: &[std::path::PathBuf]) -> Option<String> {
        let repo = super::discover(dir).unwrap();
        let workdir = super::absolute_workdir(&repo).unwrap();
        let parent = repo.head_commit().unwrap();
        super::write_commit_on_branch(&repo, &workdir, &parent, "version-packages", "chore", paths)
            .unwrap()
            .map(|id| id.to_string())
    }

    fn stock() -> Vec<String> {
        super::candidate_names(None)
    }

    fn fixture() -> (tempfile::TempDir, String) {
        let dir = tempfile::TempDir::new().unwrap();
        init(dir.path());
        let sha = commit_all(dir.path(), "init");
        (dir, sha)
    }

    fn add_origin(dir: &Path) {
        git(
            dir,
            &[
                "remote",
                "add",
                "origin",
                "https://github.com/victor-software-house/verctl.git",
            ],
        );
    }

    fn track(dir: &Path, name: &str, sha: &str) {
        git(
            dir,
            &["update-ref", &format!("refs/remotes/origin/{name}"), sha],
        );
    }

    fn set_origin_head(dir: &Path, name: &str) {
        git(
            dir,
            &[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                &format!("refs/remotes/origin/{name}"),
            ],
        );
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
        let (dir, _) = fixture();
        prove(dir.path(), &stock()).unwrap();
    }

    #[test]
    fn head_on_origin_main_is_ok() {
        let (dir, sha) = fixture();
        add_origin(dir.path());
        track(dir.path(), "main", &sha);
        prove(dir.path(), &stock()).unwrap();
    }

    #[test]
    fn head_sha_is_the_peeled_head_commit() {
        let (dir, sha) = fixture();
        assert_eq!(super::head_sha(dir.path()).unwrap(), sha);
    }

    #[test]
    fn origin_master_without_head_is_ok() {
        let (dir, sha) = fixture();
        add_origin(dir.path());
        track(dir.path(), "master", &sha);
        prove(dir.path(), &stock()).unwrap();
    }

    #[test]
    fn origin_head_to_non_main_is_ok() {
        let (dir, sha) = fixture();
        add_origin(dir.path());
        track(dir.path(), "trunk", &sha);
        set_origin_head(dir.path(), "trunk");
        prove(dir.path(), &stock()).unwrap();
    }

    #[test]
    fn base_ref_trunk_without_origin_head_is_ok() {
        let (dir, sha) = fixture();
        add_origin(dir.path());
        track(dir.path(), "trunk", &sha);
        prove(dir.path(), &super::candidate_names(Some("trunk"))).unwrap();
    }

    #[test]
    fn origin_without_default_tracking_ref_fails() {
        let (dir, _) = fixture();
        add_origin(dir.path());
        let err = prove(dir.path(), &stock()).unwrap_err();
        assert!(format!("{err:#}").contains("origin exists"), "{err:#}");
    }

    #[test]
    fn origin_trunk_alone_needs_head_or_base_ref() {
        let (dir, sha) = fixture();
        add_origin(dir.path());
        track(dir.path(), "trunk", &sha);
        let err = prove(dir.path(), &stock()).unwrap_err();
        assert!(format!("{err:#}").contains("origin exists"), "{err:#}");
        prove(dir.path(), &super::candidate_names(Some("trunk"))).unwrap();
        set_origin_head(dir.path(), "trunk");
        prove(dir.path(), &stock()).unwrap();
    }

    #[test]
    fn origin_hotfix_alone_is_not_the_default() {
        let (dir, sha) = fixture();
        add_origin(dir.path());
        track(dir.path(), "hotfix", &sha);
        let err = prove(dir.path(), &stock()).unwrap_err();
        assert!(format!("{err:#}").contains("origin exists"), "{err:#}");
    }

    #[test]
    fn unpeelable_origin_head_does_not_fall_through_to_main() {
        let (dir, sha) = fixture();
        add_origin(dir.path());
        track(dir.path(), "main", &sha);
        set_origin_head(dir.path(), "trunk");
        let err = prove(dir.path(), &stock()).unwrap_err();
        assert!(format!("{err:#}").contains("origin/trunk"), "{err:#}");
    }

    #[test]
    fn github_https_url_has_no_credentials() {
        let url = super::github_https_url("victor-software-house/qctl");
        assert_eq!(url, "https://github.com/victor-software-house/qctl.git");
        let lower = url.to_ascii_lowercase();
        assert!(!lower.contains("bearer"), "{url}");
        assert!(!lower.contains("token"), "{url}");
        assert!(!lower.contains("x-access-token"), "{url}");
    }

    #[test]
    fn github_git_user_is_x_access_token() {
        assert_eq!(super::github_git_user(), "x-access-token");
    }

    #[test]
    fn ensure_origin_head_skips_fetch_when_the_ref_exists() {
        let (dir, sha) = fixture();
        git(
            dir.path(),
            &["remote", "add", "origin", "https://127.0.0.1:1/nope.git"],
        );
        track(dir.path(), "main", &sha);
        super::ensure_origin_head(dir.path(), "main", &super::FetchDefault::Origin).unwrap();
        assert_eq!(
            git(dir.path(), &["symbolic-ref", "refs/remotes/origin/HEAD"]),
            "refs/remotes/origin/main"
        );
    }

    #[test]
    fn ensure_origin_head_fetches_a_missing_ref_from_origin() {
        let upstream_dir = tempfile::TempDir::new().unwrap();
        init(upstream_dir.path());
        commit_all(upstream_dir.path(), "init");

        let work_dir = tempfile::TempDir::new().unwrap();
        init(work_dir.path());
        commit_all(work_dir.path(), "local");
        let url = format!("file://{}", upstream_dir.path().display());
        git(work_dir.path(), &["remote", "add", "origin", &url]);

        super::ensure_origin_head(work_dir.path(), "main", &super::FetchDefault::Origin).unwrap();
        git(
            work_dir.path(),
            &["rev-parse", "--verify", "refs/remotes/origin/main^{commit}"],
        );
        assert_eq!(
            git(
                work_dir.path(),
                &["symbolic-ref", "refs/remotes/origin/HEAD"]
            ),
            "refs/remotes/origin/main"
        );
    }

    #[test]
    fn fetch_one_branch_does_not_set_depth() {
        let src = include_str!("git.rs");
        let prod = src.split("#[cfg(test)]").next().expect("production");
        assert!(
            !prod.contains("opts.depth"),
            "a depth-limited fetch leaves the tip parentless so graph_descendant_of cannot prove ancestry"
        );
    }

    #[test]
    fn configured_default_branch_is_only_verctl_default_branch() {
        let src = include_str!("git.rs");
        let start = src
            .find("pub fn configured_default_branch")
            .expect("configured_default_branch");
        let rest = &src[start..];
        let end = rest
            .find("pub fn ensure_origin_head")
            .expect("ensure_origin_head");
        let body = &rest[..end];
        assert!(!body.contains("GITHUB_BASE_REF"), "{body}");
        assert!(!body.contains("env_base_ref"), "{body}");
    }

    #[test]
    fn fetched_default_branch_keeps_parents_for_ancestry() {
        let upstream_dir = tempfile::TempDir::new().unwrap();
        init(upstream_dir.path());
        commit_all(upstream_dir.path(), "A");

        let work_dir = tempfile::TempDir::new().unwrap();
        init(work_dir.path());
        let url = format!("file://{}", upstream_dir.path().display());
        git(work_dir.path(), &["remote", "add", "origin", &url]);
        super::ensure_origin_head(work_dir.path(), "main", &super::FetchDefault::Origin).unwrap();
        let first = git(work_dir.path(), &["rev-parse", "refs/remotes/origin/main"]);
        git(
            work_dir.path(),
            &["update-ref", "--no-deref", "HEAD", &first],
        );

        std::fs::write(upstream_dir.path().join("b.txt"), "b\n").unwrap();
        commit_all(upstream_dir.path(), "B");
        std::fs::write(upstream_dir.path().join("c.txt"), "c\n").unwrap();
        commit_all(upstream_dir.path(), "C");

        git(
            work_dir.path(),
            &["update-ref", "-d", "refs/remotes/origin/main"],
        );
        git(
            work_dir.path(),
            &["symbolic-ref", "--delete", "refs/remotes/origin/HEAD"],
        );
        super::ensure_origin_head(work_dir.path(), "main", &super::FetchDefault::Origin).unwrap();
        prove(work_dir.path(), &stock()).unwrap();
    }

    #[test]
    fn origin_head_wins_over_main_when_they_differ() {
        let (dir, first) = fixture();
        add_origin(dir.path());
        std::fs::write(dir.path().join("extra.txt"), "x\n").unwrap();
        let second = commit_all(dir.path(), "ahead");
        track(dir.path(), "main", &first);
        track(dir.path(), "trunk", &second);
        set_origin_head(dir.path(), "trunk");
        prove(dir.path(), &stock()).unwrap();
    }

    #[test]
    fn head_off_default_history_fails() {
        let (dir, first) = fixture();
        add_origin(dir.path());
        track(dir.path(), "main", &first);
        git(dir.path(), &["update-ref", "refs/heads/other", &first]);
        git(dir.path(), &["symbolic-ref", "HEAD", "refs/heads/other"]);
        std::fs::write(dir.path().join("extra.txt"), "x\n").unwrap();
        commit_all(dir.path(), "ahead");
        let err = prove(dir.path(), &stock()).unwrap_err();
        assert!(format!("{err:#}").contains("not an ancestor"), "{err:#}");
    }

    #[test]
    fn head_behind_origin_main_is_ok() {
        let (dir, first) = fixture();
        add_origin(dir.path());
        std::fs::write(dir.path().join("extra.txt"), "x\n").unwrap();
        let second = commit_all(dir.path(), "ahead");
        track(dir.path(), "main", &second);
        git(dir.path(), &["update-ref", "--no-deref", "HEAD", &first]);
        prove(dir.path(), &stock()).unwrap();
    }
}
