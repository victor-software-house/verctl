use std::fs::OpenOptions;
use std::future::Future;
use std::path::Path;
use std::time::Duration;
use std::{env, fmt, fs, io};

use anyhow::{Context, Result, bail, ensure};
use octocrab::Octocrab;
use octocrab::models::repos::Object;
use octocrab::params::State;
use octocrab::params::repos::Reference;
use octocrab::repos::releases::MakeLatest;
use tokio::runtime::Builder;
use tokio::time;

use crate::{git, lookup, release, stop};

/// Append to `$GITHUB_OUTPUT`. That file belongs to the whole step, so a
/// writer that truncates would drop assignments another step already made.
/// GitHub takes the last value for a repeated key.
/// A previous writer that skipped its own trailing newline would otherwise
/// have its last line fused onto this one, losing both assignments.
///
/// # Errors
///
/// Returns an error, naming `path`, when the file cannot be opened, read,
/// or written.
pub fn write_output(path: &Path, body: &str) -> Result<()> {
    use std::io::{Read, Seek, SeekFrom, Write};
    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(path)
        .with_context(|| path.display().to_string())?;
    let end = file
        .metadata()
        .with_context(|| path.display().to_string())?
        .len();
    if end > 0 {
        let mut last = [0u8];
        file.seek(SeekFrom::Start(end - 1))
            .and_then(|_| file.read_exact(&mut last))
            .with_context(|| path.display().to_string())?;
        if last[0] != b'\n' {
            file.write_all(b"\n")
                .with_context(|| path.display().to_string())?;
        }
    }
    file.write_all(body.as_bytes())
        .with_context(|| path.display().to_string())
}

/// A GitHub repository, as `owner/name`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    /// The user or organization that owns the repository.
    pub owner: String,
    /// The repository name, without a `.git` suffix.
    pub name: String,
}

/// Actions sets `GITHUB_REPOSITORY`. Locally, `origin` is the contract.
///
/// # Errors
///
/// Returns an error when `GITHUB_REPOSITORY` is set but is not `owner/name`,
/// or, without it, when the `origin` URL cannot be read or is not a GitHub
/// remote. Without `GITHUB_REPOSITORY`, the error is a
/// [`lookup::Unavailable`]: the repository names no GitHub repository to ask.
/// A malformed `GITHUB_REPOSITORY` is a setting to fix, so it stays unmarked.
pub fn repo(root: &Path) -> Result<Repo> {
    if let Ok(slug) = env::var("GITHUB_REPOSITORY")
        && !slug.is_empty()
    {
        return parse_slug(&slug);
    }
    git::origin_url(root)
        .and_then(|url| parse_remote(url.trim()))
        .map_err(lookup::unavailable)
}

/// Parses an `owner/name` slug.
///
/// # Errors
///
/// Returns an error when `slug` has no `/`, an empty owner or name, or more
/// than one `/`.
pub fn parse_slug(slug: &str) -> Result<Repo> {
    let (owner, name) = slug
        .split_once('/')
        .context("GITHUB_REPOSITORY must be owner/name")?;
    ensure!(
        !owner.is_empty() && !name.is_empty() && !name.contains('/'),
        "GITHUB_REPOSITORY must be owner/name"
    );
    Ok(Repo {
        owner: owner.to_owned(),
        name: name.trim_end_matches(".git").to_owned(),
    })
}

/// Parses an ssh or https remote URL on github.com or a `*.github.com` host.
///
/// # Errors
///
/// Returns an error when the URL is not an ssh or http(s) URL with a path.
/// It also returns one when its host is not GitHub or the path is not
/// `owner/name`.
pub fn parse_remote(url: &str) -> Result<Repo> {
    let url = url.trim();
    if let Some(rest) = url.strip_prefix("git@") {
        let (host, path) = rest
            .split_once(':')
            .context("ssh remote must be git@host:owner/name")?;
        ensure!(
            host == "github.com" || host.ends_with(".github.com"),
            "origin is not GitHub ({host})"
        );
        return parse_slug(path);
    }
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .or_else(|| url.strip_prefix("ssh://git@"))
        .context("origin URL is not ssh or https")?;
    let (host, path) = rest.split_once('/').context("origin URL has no path")?;
    ensure!(
        host == "github.com" || host.ends_with(".github.com"),
        "origin is not GitHub ({host})"
    );
    parse_slug(path)
}

/// The branch a Version PR targets: `GITHUB_REF_NAME`, else
/// `VERCTL_DEFAULT_BRANCH`, else the branch `origin/HEAD` names.
///
/// If none of them is set, the call fails instead of guessing.
/// A repository whose default is not `main` would otherwise get a Version PR
/// against a branch it does not release from.
///
/// # Errors
///
/// Returns an error when none of those three sources names a branch.
pub fn base_branch(root: &Path) -> Result<String> {
    if let Ok(name) = env::var("GITHUB_REF_NAME")
        && !name.is_empty()
    {
        return Ok(name);
    }
    git::configured_default_branch()
        .or_else(|| git::upstream_default_branch(root))
        .context(
            "cannot tell which branch the Version PR targets: set VERCTL_DEFAULT_BRANCH, or run `git remote set-head origin --auto`",
        )
}

/// An open pull request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExistingPr {
    /// Link to the pull request.
    pub url: String,
    /// The pull request number.
    pub number: u64,
}

/// The open pull request from `head` in this repository, if one exists.
///
/// # Errors
///
/// Returns an error when the GitHub client cannot be built or the call that
/// lists pull requests fails.
pub fn existing_pr(token: &str, repo: &Repo, head: &str) -> Result<Option<ExistingPr>> {
    block(async {
        let crab = client(token)?;
        let page = crab
            .pulls(&repo.owner, &repo.name)
            .list()
            .state(State::Open)
            .head(format!("{}:{head}", repo.owner))
            .per_page(1)
            .send()
            .await
            .context("list pull requests")?;
        Ok(page.items.into_iter().next().and_then(|pr| {
            let url = pr.html_url?.to_string();
            Some(ExistingPr {
                url,
                number: pr.number,
            })
        }))
    })
}

/// Sets the title and body of pull request `number` and returns its URL.
///
/// # Errors
///
/// Returns an error when the GitHub client cannot be built, the update
/// request fails, or the response carries no URL.
pub fn update_pr(token: &str, repo: &Repo, number: u64, title: &str, body: &str) -> Result<String> {
    block(async {
        let crab = client(token)?;
        let pr = crab
            .pulls(&repo.owner, &repo.name)
            .update(number)
            .title(title)
            .body(body)
            .send()
            .await
            .context("update pull request")?;
        pr.html_url
            .map(|url| url.to_string())
            .context("updated PR has no html_url")
    })
}

/// Opens a pull request from `head` into `base`.
///
/// # Errors
///
/// Returns an error when the GitHub client cannot be built or the create
/// request fails, for example when `head` does not exist.
/// It also returns one when the response carries no URL.
pub fn create_pr(
    token: &str,
    repo: &Repo,
    title: &str,
    head: &str,
    base: &str,
    body: &str,
) -> Result<ExistingPr> {
    block(async {
        let crab = client(token)?;
        let pr = crab
            .pulls(&repo.owner, &repo.name)
            .create(title, head, base)
            .body(body)
            .send()
            .await
            .context("create pull request")?;
        let url = pr
            .html_url
            .map(|url| url.to_string())
            .context("created PR has no html_url")?;
        Ok(ExistingPr {
            url,
            number: pr.number,
        })
    })
}

/// Whether the pull request of this `pull_request` event is a Version PR by
/// the identity of the `version-pr-identity` design §1.
/// Its head repository is this repository, and its head branch is
/// `verctl/release/<base>` for the base it targets now.
///
/// The check reads all three through the API when it runs.
/// It never reads the event payload, because a base can change after the
/// event and a label applied after opening is missing from the `opened`
/// payload.
/// It never reads the title, body, or labels, and it ignores
/// `GITHUB_HEAD_REF`.
///
/// Without an event, or for an event that is not a pull request's, it is
/// `false`.
///
/// # Errors
///
/// Returns an error when the event names a pull request but no token is set.
/// It also returns one when `GITHUB_REPOSITORY` is not `owner/name` or GitHub
/// cannot read the pull request.
pub fn pull_request_is_version_pr() -> Result<bool> {
    let Ok(path) = env::var("GITHUB_EVENT_PATH") else {
        return Ok(false);
    };
    let Ok(raw) = fs::read_to_string(path) else {
        return Ok(false);
    };
    let Some(number) = pull_request_number(&raw) else {
        return Ok(false);
    };
    let token = release::env_token().context(
        "the version check reads the pull request and needs GITHUB_TOKEN with pull-requests: read",
    )?;
    let slug = env::var("GITHUB_REPOSITORY").unwrap_or_default();
    let repo = parse_slug(&slug)?;
    block(async move {
        let crab = client(&token)?;
        let pr = crab
            .pulls(&repo.owner, &repo.name)
            .get(number)
            .await
            .with_context(|| format!("read pull request #{number}"))?;
        let same_repo = pr
            .head
            .repo
            .as_ref()
            .and_then(|head| head.full_name.as_deref())
            .is_some_and(|name| name.eq_ignore_ascii_case(&slug));
        Ok(same_repo && pr.head.ref_field == release::version_branch(&pr.base.ref_field))
    })
}

/// The pull request number of a `pull_request` event payload, or `None` when
/// it does not parse or is another event's.
#[must_use]
pub fn pull_request_number(raw: &str) -> Option<u64> {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()?
        .get("pull_request")?
        .get("number")?
        .as_u64()
}

/// Whether the Actions event is a merge queue's `merge_group` event, read
/// from its payload. Without an event it is `false`.
#[must_use]
pub fn event_is_merge_group() -> bool {
    env::var("GITHUB_EVENT_PATH")
        .ok()
        .and_then(|path| fs::read_to_string(path).ok())
        .is_some_and(|raw| is_merge_group(&raw))
}

/// Whether an event payload is a `merge_group` event's.
#[must_use]
pub fn is_merge_group(raw: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|event| event.get("merge_group").map(serde_json::Value::is_object))
        .unwrap_or(false)
}

/// The repository's default branch named by the Actions event payload, or
/// `None` without an event.
#[must_use]
pub fn event_default_branch() -> Option<String> {
    let raw = fs::read_to_string(env::var("GITHUB_EVENT_PATH").ok()?).ok()?;
    default_branch_of(&raw)
}

/// `repository.default_branch` of an event payload, or `None` when it does
/// not parse or names none.
#[must_use]
pub fn default_branch_of(raw: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()?
        .get("repository")?
        .get("default_branch")?
        .as_str()
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
}

/// Where a Release stands: the stable one, or a candidate before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// A stable version: GitHub's defaults apply, so the Release becomes the
    /// repository's latest.
    Stable,
    /// A candidate: a prerelease that never becomes the latest Release, so
    /// the repository's latest stays the stable one ([create a release]).
    ///
    /// [create a release]: https://docs.github.com/en/rest/releases/releases#create-a-release
    Candidate,
}

/// The Release a run creates for a tag: a draft for `release`, or a Release
/// published at once for [`ensure_release`].
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct NewRelease<'a> {
    /// The tag the Release is for.
    pub tag: &'a str,
    /// The Release title.
    pub name: &'a str,
    /// The Release notes.
    pub body: &'a str,
    /// The commit the tag is created at, and the one an existing tag must name.
    pub sha: &'a str,
    /// Whether the Release is the stable one or a candidate.
    pub stage: Stage,
    /// The asset names the draft must hold before it is published, because
    /// an immutable Release takes no asset once published ([immutable
    /// releases]). Empty, the draft is published with none.
    ///
    /// [immutable releases]: https://docs.github.com/en/code-security/supply-chain-security/understanding-your-software-supply-chain/immutable-releases
    pub assets: &'a [String],
}

/// Create `tag` at `sha` if it is missing. Returns the release HTML URL.
///
/// A new Release is a [`Stage::Stable`] one, with GitHub's defaults, and is
/// published at once. An existing release is used as it is, and a draft found
/// is published.
///
/// An existing release is not trusted: GitHub's `target_commitish` on the
/// Release object is a branch name more often than a SHA. The git ref is
/// re-read either way, and a tag that names another commit fails.
///
/// # Errors
///
/// Returns an error when:
///
/// - The GitHub client cannot be built.
/// - A release or tag lookup fails.
/// - The release cannot be created, or a draft found cannot be published.
/// - The tag does not point at `sha`.
///
/// The tag is checked against an existing release only when `shipped` is
/// true.
pub fn ensure_release(
    token: &str,
    repo: &Repo,
    tag: &str,
    name: &str,
    body: &str,
    sha: &str,
    shipped: bool,
) -> Result<String> {
    let release = NewRelease {
        tag,
        name,
        body,
        sha,
        stage: Stage::Stable,
        assets: &[],
    };
    block(async {
        let crab = client(token)?;
        ensure_release_with(&crab, repo, &release, shipped).await
    })
}

/// `shipped` says this run published a package under the tag. When nothing
/// under it shipped, an existing published release belongs to the run that
/// did, and its tag may name that earlier commit. A draft's tag is always
/// checked.
async fn ensure_release_with(
    crab: &Octocrab,
    repo: &Repo,
    release: &NewRelease<'_>,
    shipped: bool,
) -> Result<String> {
    let NewRelease {
        tag,
        name,
        body,
        sha,
        stage,
        ..
    } = *release;
    let existing = if read_tag(crab, repo, tag).await?.is_some() {
        find_release(crab, repo, tag).await?
    } else {
        None
    };
    match existing {
        Some(found) if !found.draft => {
            if shipped {
                require_tag_at(crab, repo, tag, sha).await?;
            }
            return Ok(found.html_url);
        }
        Some(draft) => {
            require_tag_at(crab, repo, tag, sha).await?;
            return finalize(crab, repo, release, &draft).await;
        }
        None => {}
    }
    let repos = crab.repos(&repo.owner, &repo.name);
    let releases = repos.releases();
    let create = releases.create(tag).name(name).body(body);
    let create = match stage {
        Stage::Stable => create,
        Stage::Candidate => create.prerelease(true).make_latest(MakeLatest::False),
    };
    let created = create
        .target_commitish(sha)
        .send()
        .await
        .map_err(|error| release_error(error, tag, sha))?;
    require_tag_at(crab, repo, tag, sha).await?;
    Ok(created.html_url.to_string())
}

/// How long the tag proof waits before each read after the first, while
/// GitHub answers 404 for a tag that publishing the draft created.
/// Five reads over 15 seconds, inside the 30 seconds the release-tag-order
/// spec allows: GitHub can serve a new tag a moment after the Release that
/// created it, as in rawr run 43414046487.
pub const TAG_PROOF_DELAYS: [Duration; 4] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
    Duration::from_secs(8),
];

/// What a run finds on GitHub for one release tag, before it writes
/// anything.
#[derive(Debug)]
pub(crate) enum Found {
    /// A Release published at HEAD. The run opens no draft for it and proves
    /// its tag. A mutable one takes the declared tarballs it lacks, so an
    /// older release missing a target can still be finished.
    Published(ForgeRelease),
    /// No published Release: the run opens a draft for the tag.
    Unpublished(Unpublished),
}

impl Found {
    /// The asset names already on the Release the run uploads to, or `None`
    /// for a published immutable Release, which takes no asset ([immutable
    /// releases]). Its shortfall is reported, and nothing is uploaded.
    ///
    /// [immutable releases]: https://docs.github.com/en/code-security/supply-chain-security/understanding-your-software-supply-chain/immutable-releases
    pub(crate) fn uploaded(&self) -> Option<Vec<&str>> {
        match self {
            Self::Published(release) if release.immutable => None,
            Self::Published(release) => Some(release.uploaded()),
            Self::Unpublished(draft) => Some(draft.uploaded()),
        }
    }
}

/// A tag with no published Release, and the draft a run finds for it.
#[derive(Debug)]
pub(crate) enum Unpublished {
    /// No Release exists for the tag.
    Missing,
    /// A draft at HEAD, which the run uploads to and publishes.
    AtHead(ForgeRelease),
    /// A draft at another commit or at a branch name. Its tarballs were built
    /// from other code, so the run deletes it and creates a draft at HEAD.
    Stale(ForgeRelease),
}

impl Unpublished {
    /// The asset names uploaded on the draft the run keeps. A stale draft is
    /// deleted, so nothing on it counts.
    pub(crate) fn uploaded(&self) -> Vec<&str> {
        match self {
            Self::AtHead(draft) => draft.uploaded(),
            Self::Missing | Self::Stale(_) => Vec::new(),
        }
    }
}

/// Read what GitHub holds for `tag`: the tag and its Release, draft or
/// published, so a run reads what to create before it writes anything.
///
/// A tag that exists must name `head`, whatever the Release's state: GitHub
/// uses an existing tag when a draft is published, so a tag on another commit
/// would fail the tag proof after the uploads.
/// A published Release counts as at HEAD when its tag names `head`, or when
/// GitHub serves no tag yet and the Release's `target_commitish` is `head`.
/// A draft counts as at HEAD when its `target_commitish` is `head`, or when
/// its tag names `head`.
///
/// # Errors
///
/// Returns an error when the GitHub client cannot be built, when a lookup
/// fails, when the tag names a commit other than `head`, or when a Release is
/// published at another commit.
pub(crate) fn read_release(token: &str, repo: &Repo, tag: &str, head: &str) -> Result<Found> {
    block(async {
        let crab = client(token)?;
        read_release_with(&crab, repo, tag, head).await
    })
}

async fn read_release_with(crab: &Octocrab, repo: &Repo, tag: &str, head: &str) -> Result<Found> {
    let named = read_tag(crab, repo, tag).await?;
    if let Some(named) = &named {
        ensure!(
            named.eq_ignore_ascii_case(head),
            "tag {tag} names {named}, not HEAD {head}"
        );
    }
    let Some(release) = find_release(crab, repo, tag).await? else {
        return Ok(Found::Unpublished(Unpublished::Missing));
    };
    let at_head = named.is_some() || release.target_commitish.eq_ignore_ascii_case(head);
    if release.draft {
        return Ok(Found::Unpublished(if at_head {
            Unpublished::AtHead(release)
        } else {
            Unpublished::Stale(release)
        }));
    }
    ensure!(
        at_head,
        "release {tag} is published at {target}, not HEAD {head}, and GitHub serves no tag {tag}",
        target = release.target_commitish
    );
    Ok(Found::Published(release))
}

/// The id of the draft for `release`: the one `found` names at HEAD, or a new
/// draft with `target_commitish` set to `release.sha`.
/// A stale draft is deleted first.
///
/// The draft creates no tag: GitHub creates it at `target_commitish` when the
/// draft is published ([create a release]).
/// Until then the draft can still be changed or deleted, so a failed upload
/// leaves no tag behind.
///
/// # Errors
///
/// Returns an error when the GitHub client cannot be built, a stale draft
/// cannot be deleted, or the draft cannot be created.
/// A 403 on the create names the missing permission.
///
/// [create a release]: https://docs.github.com/en/rest/releases/releases#create-a-release
pub(crate) fn open_draft(
    token: &str,
    repo: &Repo,
    release: &NewRelease<'_>,
    found: &Unpublished,
) -> Result<u64> {
    block(async {
        let crab = client(token)?;
        open_draft_with(&crab, repo, release, found).await
    })
}

async fn open_draft_with(
    crab: &Octocrab,
    repo: &Repo,
    release: &NewRelease<'_>,
    found: &Unpublished,
) -> Result<u64> {
    let NewRelease {
        tag,
        name,
        body,
        sha,
        stage,
        ..
    } = *release;
    let repos = crab.repos(&repo.owner, &repo.name);
    let releases = repos.releases();
    match found {
        Unpublished::AtHead(draft) => return Ok(draft.id),
        Unpublished::Stale(draft) => releases.delete(draft.id).await.with_context(|| {
            format!(
                "delete the draft release {tag} at {target}",
                target = draft.target_commitish
            )
        })?,
        Unpublished::Missing => {}
    }
    let create = releases
        .create(tag)
        .name(name)
        .body(body)
        .draft(true)
        .target_commitish(sha);
    let create = match stage {
        Stage::Stable => create,
        Stage::Candidate => create.prerelease(true).make_latest(MakeLatest::False),
    };
    let created = create
        .send()
        .await
        .map_err(|error| release_error(error, tag, sha))?;
    Ok(created.id.into_inner())
}

/// Publish the draft `id` for `release`, which creates its tag at
/// `release.sha`, and return the Release URL.
///
/// The draft is read again first, so the check sees what GitHub holds: a
/// draft that lacks a declared asset is never published.
///
/// # Errors
///
/// Returns an error when the GitHub client cannot be built, the draft cannot
/// be read or published, or it lacks a declared asset.
pub(crate) fn publish_draft(
    token: &str,
    repo: &Repo,
    release: &NewRelease<'_>,
    id: u64,
) -> Result<String> {
    block(async {
        let crab = client(token)?;
        publish_draft_with(&crab, repo, release, id).await
    })
}

async fn publish_draft_with(
    crab: &Octocrab,
    repo: &Repo,
    release: &NewRelease<'_>,
    id: u64,
) -> Result<String> {
    let draft: ForgeRelease = crab
        .get(
            format!(
                "/repos/{owner}/{name}/releases/{id}",
                owner = repo.owner,
                name = repo.name
            ),
            None::<&()>,
        )
        .await
        .with_context(|| format!("read the draft release {}", release.tag))?;
    finalize(crab, repo, release, &draft).await
}

/// Write `body` as the notes of draft `id` of `tag`, and nothing else, so
/// the draft keeps its state: a failed run records why it stopped there.
///
/// # Errors
///
/// Returns an error when the GitHub client cannot be built or the update
/// fails.
pub(crate) fn write_draft_body(
    token: &str,
    repo: &Repo,
    tag: &str,
    id: u64,
    body: &str,
) -> Result<()> {
    block(async {
        let crab = client(token)?;
        crab.repos(&repo.owner, &repo.name)
            .releases()
            .update(id)
            .body(body)
            .send()
            .await
            .with_context(|| format!("write the stop record into the draft release {tag}"))?;
        Ok(())
    })
}

/// Publish `draft` once every asset `release` declares is `uploaded` on it,
/// and return its URL.
///
/// The update sends the title and notes this run holds, `target_commitish`,
/// and the settings its stage sets, so a draft someone edited reads as this
/// run would have written it.
/// A candidate stays a prerelease and never becomes the latest.
/// A stable one sends `make_latest: legacy`, so GitHub decides by date and
/// version, and an older version published late does not take the latest from
/// a newer one ([update a release]).
///
/// [update a release]: https://docs.github.com/en/rest/releases/releases#update-a-release
async fn finalize(
    crab: &Octocrab,
    repo: &Repo,
    release: &NewRelease<'_>,
    draft: &ForgeRelease,
) -> Result<String> {
    let uploaded = draft.uploaded();
    let missing: Vec<&str> = release
        .assets
        .iter()
        .map(String::as_str)
        .filter(|name| !uploaded.contains(name))
        .collect();
    ensure!(
        missing.is_empty(),
        "the draft release {tag} lacks {missing}, so it stays a draft; rerun release with them in --assets-dir, or with --skip-assets to publish it without them",
        tag = draft.tag_name,
        missing = missing.join(", ")
    );
    let repos = crab.repos(&repo.owner, &repo.name);
    let releases = repos.releases();
    let update = releases
        .update(draft.id)
        .name(release.name)
        .body(release.body)
        .target_commitish(release.sha)
        .draft(false);
    let update = match release.stage {
        Stage::Stable => update.make_latest(MakeLatest::Legacy),
        Stage::Candidate => update.prerelease(true).make_latest(MakeLatest::False),
    };
    let published = update
        .send()
        .await
        .with_context(|| format!("publish the draft release {}", draft.tag_name))?;
    Ok(published.html_url.to_string())
}

/// Prove that `tag` names `head`, reading it again after each of `delays`
/// while GitHub answers 404.
///
/// Publishing a draft creates its tag, and GitHub can serve the tag a moment
/// after the Release. Only a 404 is read again: any other error, and a tag
/// that names another commit, fail at once.
///
/// # Errors
///
/// Returns an error when the GitHub client cannot be built, a read fails with
/// anything but 404, the tag names a commit other than `head`, or every read
/// answers 404. The last names the tag and the number of reads.
pub(crate) fn prove_tag(
    token: &str,
    repo: &Repo,
    tag: &str,
    head: &str,
    delays: &[Duration],
) -> Result<()> {
    block(async {
        let crab = client(token)?;
        prove_tag_with(&crab, repo, tag, head, delays).await
    })
}

async fn prove_tag_with(
    crab: &Octocrab,
    repo: &Repo,
    tag: &str,
    head: &str,
    delays: &[Duration],
) -> Result<()> {
    let mut waits = delays.iter();
    let mut reads = 0_usize;
    loop {
        reads += 1;
        if let Some(named) = read_tag(crab, repo, tag).await? {
            ensure!(
                named.eq_ignore_ascii_case(head),
                "tag {tag} names {named}, not HEAD {head}"
            );
            return Ok(());
        }
        let Some(wait) = waits.next() else {
            bail!(
                "GitHub serves no tag {tag} after {reads} reads, though its Release is published; rerun release to read the tag again"
            );
        };
        time::sleep(*wait).await;
    }
}

/// The Release for a vendored set's `tag`, created at `sha` when the tag has
/// none. Returns its HTML URL.
///
/// An existing Release is used as it is.
/// Its tag records which commit built the first asset, and that commit is
/// not a version commit, so the tag target is not checked.
///
/// A new Release is created with `make_latest: false`.
/// GitHub otherwise makes every new published Release the latest of the
/// repository ([create a release]).
/// A vendored library must not displace the Release of the repository itself.
///
/// # Errors
///
/// Returns an error when the GitHub client cannot be built or when reading the
/// Release fails with anything but 404.
/// It also returns one when creating the Release fails for any reason other
/// than another job having created it first.
///
/// [create a release]: https://docs.github.com/en/rest/releases/releases#create-a-release
pub fn ensure_vendored_release(
    token: &str,
    repo: &Repo,
    tag: &str,
    body: &str,
    sha: &str,
) -> Result<String> {
    block(async {
        let crab = client(token)?;
        ensure_vendored_release_with(&crab, repo, tag, body, sha).await
    })
}

/// Every target's job runs this, so two can find the Release missing at once.
/// GitHub refuses one of them with `already_exists`, and that job reads the
/// Release the other created.
async fn ensure_vendored_release_with(
    crab: &Octocrab,
    repo: &Repo,
    tag: &str,
    body: &str,
    sha: &str,
) -> Result<String> {
    let repos = crab.repos(&repo.owner, &repo.name);
    let releases = repos.releases();
    match releases.get_by_tag(tag).await {
        Ok(release) => return Ok(release.html_url.to_string()),
        Err(error) if is_not_found(&error) => {}
        Err(error) => return Err(error).with_context(|| format!("read release {tag}")),
    }
    let created = releases
        .create(tag)
        .name(tag)
        .body(body)
        .target_commitish(sha)
        .make_latest(MakeLatest::False)
        .send()
        .await;
    match created {
        Ok(release) => Ok(release.html_url.to_string()),
        Err(error) if is_already_exists(&error) => {
            let release = releases
                .get_by_tag(tag)
                .await
                .with_context(|| format!("read release {tag}, which another job created"))?;
            Ok(release.html_url.to_string())
        }
        Err(error) => Err(release_error(error, tag, sha)),
    }
}

/// The hex SHA-256 that GitHub holds for asset `name` in the `tag` Release.
/// It comes from the `digest` of the asset (`sha256:<hex>`, see
/// [release assets]).
/// Reading the digest from GitHub, instead of hashing a local file, keeps the
/// checksum matching the stored asset.
/// A rerun's build can differ in bytes from the one GitHub kept.
///
/// # Errors
///
/// Returns an error when the GitHub client cannot be built or the Release
/// cannot be read.
/// It also returns one when the Release does not list `name` or the asset has
/// no SHA-256 digest.
///
/// [release assets]: https://docs.github.com/en/rest/releases/assets
pub fn asset_sha256(token: &str, repo: &Repo, tag: &str, name: &str) -> Result<String> {
    block(async {
        let crab = client(token)?;
        asset_sha256_with(&crab, repo, tag, name).await
    })
}

async fn asset_sha256_with(crab: &Octocrab, repo: &Repo, tag: &str, name: &str) -> Result<String> {
    let release = upload_target(crab, repo, tag).await?;
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == name)
        .with_context(|| format!("release {tag} does not list asset {name}"))?;
    asset
        .digest
        .as_deref()
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .map(str::to_owned)
        .with_context(|| format!("GitHub reports no SHA-256 digest for asset {name} of {tag}"))
}

/// Names the cause of GitHub's 403 for a new Release.
/// Creating a Release takes `contents: write`, plus `workflows: write` when
/// the commit the tag would point at has a `.github/workflows` tree that
/// differs from the default branch.
/// The `X-Accepted-GitHub-Permissions` header of the refusal lists both sets.
///
/// An Actions `GITHUB_TOKEN` can never hold `workflows`, so releasing an
/// older version commit fails once a later commit changes a workflow.
fn release_error(error: octocrab::Error, tag: &str, sha: &str) -> anyhow::Error {
    let forbidden = matches!(
        &error,
        octocrab::Error::GitHub { source, .. } if source.status_code.as_u16() == 403
    );
    let error = anyhow::Error::new(error).context("create GitHub release");
    if forbidden {
        error.context(ReleaseRefused {
            tag: tag.to_owned(),
            sha: sha.to_owned(),
        })
    } else {
        error
    }
}

/// GitHub's 403 on creating a Release.
/// The token lacks a permission the commit needs.
/// A rerun with a token that has the permission can still create the Release.
#[derive(Debug)]
struct ReleaseRefused {
    tag: String,
    sha: String,
}

impl fmt::Display for ReleaseRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "GitHub refused to create {tag} at {sha}. When that commit's .github/workflows differ from the default branch, the Releases API also needs workflows: write, which GITHUB_TOKEN cannot hold; rerun verctl release at that commit with a token that has it",
            tag = self.tag,
            sha = self.sha
        )
    }
}

async fn require_tag_at(crab: &Octocrab, repo: &Repo, tag: &str, sha: &str) -> Result<()> {
    let named = read_tag(crab, repo, tag)
        .await?
        .with_context(|| format!("tag {tag} does not exist"))?;
    ensure!(
        named.eq_ignore_ascii_case(sha),
        "tag {tag} names {named}, not the published commit {sha}"
    );
    Ok(())
}

/// The commit `tag` names, peeled through an annotated tag, or `None` when
/// GitHub answers 404 for it.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "octocrab's ref Object is non_exhaustive, so a catch-all arm is required"
)]
async fn read_tag(crab: &Octocrab, repo: &Repo, tag: &str) -> Result<Option<String>> {
    let git_ref = match crab
        .repos(&repo.owner, &repo.name)
        .get_ref(&Reference::Tag(tag.to_owned()))
        .await
    {
        Ok(git_ref) => git_ref,
        Err(error) if is_not_found(&error) => return Ok(None),
        Err(error) => return Err(lookup_failed(error, format!("read tag {tag}"))),
    };
    match git_ref.object {
        Object::Commit { sha, .. } => Ok(Some(sha)),
        Object::Tag { sha: tag_sha, .. } => peel_tag(crab, repo, tag, &tag_sha).await.map(Some),
        other => bail!("tag {tag} names {other:?}, not a commit"),
    }
}

#[derive(serde::Deserialize)]
struct PeeledTag {
    object: PeeledTarget,
}

#[derive(serde::Deserialize)]
struct PeeledTarget {
    sha: String,
}

async fn peel_tag(crab: &Octocrab, repo: &Repo, tag: &str, tag_sha: &str) -> Result<String> {
    let peeled: PeeledTag = crab
        .get(
            format!(
                "/repos/{owner}/{name}/git/tags/{tag_sha}",
                owner = repo.owner,
                name = repo.name
            ),
            None::<&()>,
        )
        .await
        .map_err(|error| lookup_failed(error, format!("peel tag {tag}")))?;
    Ok(peeled.object.sha)
}

/// What GitHub holds for one release tag.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ReleaseState {
    /// The commit the tag names, peeled through an annotated tag. `None` when
    /// the tag does not exist.
    pub tag: Option<String>,
    /// Whether a Release exists for the tag, draft or published.
    pub release: bool,
    /// Whether that Release is a draft, which `release` publishes after its
    /// uploads.
    pub draft: bool,
    /// Whether GitHub holds that Release immutable: once published, its
    /// assets and tag cannot change ([immutable releases]).
    ///
    /// [immutable releases]: https://docs.github.com/en/code-security/supply-chain-security/understanding-your-software-supply-chain/immutable-releases
    pub immutable: bool,
    /// The file names of the Release's assets whose upload completed.
    pub assets: Vec<String>,
    /// The stop record of a draft's body, which the last `release` run that
    /// failed an upload wrote. Display only; a published Release has none.
    pub stop: stop::Record,
}

/// Read the tag and the Release for `tag`, so a plan can tell which parts of a
/// release still need doing.
/// The Release is read even when the tag is missing: a draft has no tag
/// until it is published, so a tagless draft is an incomplete release.
///
/// # Errors
///
/// Returns an error when the GitHub client cannot be built or when GitHub
/// answers a tag or Release lookup with an error other than 404.
/// It also returns one when the tag points at a tree or a blob.
///
/// Only a lookup that GitHub gave no answer to, meaning a transport failure
/// or a 5xx, is a [`lookup::Unavailable`].
/// A 401 from a bad token, a tag on a tree, and a malformed `GITHUB_API_URL`
/// stay unmarked.
pub fn release_state(token: &str, repo: &Repo, tag: &str) -> Result<ReleaseState> {
    block(async {
        let crab = client(token)?;
        release_state_with(&crab, repo, tag).await
    })
}

async fn release_state_with(crab: &Octocrab, repo: &Repo, tag: &str) -> Result<ReleaseState> {
    let tag_sha = read_tag(crab, repo, tag).await?;
    let release = find_release(crab, repo, tag).await?;
    Ok(ReleaseState {
        tag: tag_sha,
        release: release.is_some(),
        draft: release.as_ref().is_some_and(|release| release.draft),
        immutable: release.as_ref().is_some_and(|release| release.immutable),
        assets: release
            .as_ref()
            .map_or_default(|release| release.uploaded().into_iter().map(str::to_owned).collect()),
        stop: release
            .as_ref()
            .filter(|release| release.draft)
            .map_or_default(|release| stop::read(release.body.as_deref().unwrap_or_default())),
    })
}

/// `error` under `context`, marked as a [`lookup::Unavailable`] when GitHub
/// gave no answer.
/// That covers two cases.
/// The request failed in transport (DNS, connect, a timeout, a connection
/// dropped while the body was read).
/// Or GitHub answered with a 5xx or a rate limit after the retries of
/// octocrab.
/// [`lookup::no_answer`] separates those statuses from the rest.
///
/// Any other answer, such as a 401 or a plain 403, stays an error.
///
/// octocrab 0.54.5 turns every non-2xx answer into `GitHub` with its status
/// and headers, whatever the body holds.
/// Its `map_github_error` falls back to the body text when the body is not
/// GitHub JSON.
/// An HTML 502 from a proxy therefore keeps its status.
///
/// A transport failure reaches octocrab as `Service` from the HTTP client.
/// It also arrives as `Hyper` while octocrab reads a body, alone or inside
/// `Service`.
/// `Service` also carries a request that the client blocks before sending.
/// It counts only when an I/O or body error caused it.
fn lookup_failed(error: octocrab::Error, context: String) -> anyhow::Error {
    let refused = matches!(&error, octocrab::Error::GitHub { source, .. }
        if lookup::no_answer(source.status_code, source.headers.as_ref()));
    let body = matches!(&error, octocrab::Error::Hyper { .. });
    let service = matches!(&error, octocrab::Error::Service { .. });
    let error = anyhow::Error::new(error);
    let transport = service
        && error.chain().any(|cause| {
            cause.is::<io::Error>()
                || matches!(
                    cause.downcast_ref::<octocrab::Error>(),
                    Some(octocrab::Error::Hyper { .. })
                )
        });
    let error = error.context(context);
    if refused || body || transport {
        lookup::unavailable(error)
    } else {
        error
    }
}

fn is_not_found(error: &octocrab::Error) -> bool {
    matches!(error, octocrab::Error::GitHub { source, .. } if source.status_code.as_u16() == 404)
}

/// The part of a Release verctl reads, draft or published.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct ForgeRelease {
    id: u64,
    html_url: String,
    tag_name: String,
    /// The commit or branch the tag is created at when the Release is
    /// published. GitHub reports it for every Release. A server that does not
    /// report it names no commit, so a draft without it is stale.
    #[serde(default)]
    target_commitish: String,
    draft: bool,
    /// Reported by GitHub for every Release. A server that does not report
    /// it holds the Release mutable.
    #[serde(default)]
    immutable: bool,
    /// The notes. GitHub reports `null` for a Release with none.
    #[serde(default)]
    body: Option<String>,
    assets: Vec<ForgeAsset>,
}

impl ForgeRelease {
    /// The Release's HTML URL.
    pub(crate) fn url(&self) -> &str {
        &self.html_url
    }

    /// The names of the assets whose upload completed.
    pub(crate) fn uploaded(&self) -> Vec<&str> {
        self.assets
            .iter()
            .filter(|asset| asset.state == "uploaded")
            .map(|asset| asset.name.as_str())
            .collect()
    }
}

#[derive(Debug, serde::Deserialize)]
struct ForgeAsset {
    name: String,
    /// `uploaded` once GitHub holds the whole file; an upload that broke off
    /// leaves the asset in another state.
    state: String,
    /// `sha256:<hex>` of the stored file. GitHub reports `null` for an asset
    /// it holds no digest for.
    #[serde(default)]
    digest: Option<String>,
}

/// Releases read per page; GitHub's maximum.
const RELEASES_PER_PAGE: usize = 100;

/// Pages of Releases a lookup reads before it stops, so a repository with
/// thousands of Releases costs ten requests instead of one per page.
/// A draft verctl creates is the newest Release, so it is on the first page.
const MAX_RELEASE_PAGES: u32 = 10;

/// The Release for `tag`, draft or published, or `None`.
///
/// `GET /releases/tags/{tag}` answers for a published Release in one request.
/// It returns 404 for a draft ([get a release by tag name]).
/// Only then is the tag looked up in `GET /releases`, which lists drafts to a
/// token with push access ([list releases]).
/// The list is newest first, so a draft created a moment ago is on the first
/// page.
/// A draft has no tag until it is published, so the lookup runs whether or
/// not the tag exists.
/// A tag with no Release at all reads every page, up to [`MAX_RELEASE_PAGES`],
/// and then counts as having none: a draft older than the newest 1000
/// Releases is not found.
///
/// [get a release by tag name]: https://docs.github.com/en/rest/releases/releases#get-a-release-by-tag-name
/// [list releases]: https://docs.github.com/en/rest/releases/releases#list-releases
async fn find_release(crab: &Octocrab, repo: &Repo, tag: &str) -> Result<Option<ForgeRelease>> {
    let published: Result<ForgeRelease, _> = crab
        .get(
            format!(
                "/repos/{owner}/{name}/releases/tags/{tag}",
                owner = repo.owner,
                name = repo.name
            ),
            None::<&()>,
        )
        .await;
    match published {
        Ok(found) => return Ok(Some(found)),
        Err(error) if is_not_found(&error) => {}
        Err(error) => return Err(lookup_failed(error, format!("get release {tag}"))),
    }
    let mut page = 1_u32;
    loop {
        let releases: Vec<ForgeRelease> = crab
            .get(
                format!(
                    "/repos/{owner}/{name}/releases?per_page={RELEASES_PER_PAGE}&page={page}",
                    owner = repo.owner,
                    name = repo.name
                ),
                None::<&()>,
            )
            .await
            .map_err(|error| lookup_failed(error, format!("list releases for {tag}")))?;
        let last = releases.len() < RELEASES_PER_PAGE;
        if let Some(found) = releases.into_iter().find(|release| release.tag_name == tag) {
            return Ok(Some(found));
        }
        if last || page == MAX_RELEASE_PAGES {
            return Ok(None);
        }
        page += 1;
    }
}

/// Attach `tarball` to the release for `tag`.
///
/// Returns the download URL, or `already <name>` when GitHub refuses the
/// upload because the release already has a complete asset of that name.
/// The run that got there first, or an earlier attempt of this one, uploaded
/// it.
///
/// # Errors
///
/// Returns an error when:
///
/// - `tarball` has no UTF-8 file name or cannot be read.
/// - The GitHub client cannot be built.
/// - The release for `tag` does not exist.
/// - The asset of that name exists but its upload does not complete within a
///   minute.
/// - The upload fails for a reason other than a duplicate asset.
pub fn upload_release_asset(token: &str, repo: &Repo, tag: &str, tarball: &Path) -> Result<String> {
    let name = tarball
        .file_name()
        .and_then(|name| name.to_str())
        .context("asset name")?
        .to_owned();
    let body =
        bytes::Bytes::from(fs::read(tarball).with_context(|| tarball.display().to_string())?);
    block(async {
        let crab = client(token)?;
        upload_release_asset_with(&crab, repo, tag, &name, body, &UPLOAD_SETTLE_DELAYS).await
    })
}

/// How long an upload refused as a duplicate waits for the other upload to
/// finish. GitHub lists an asset as soon as its upload starts, in a state
/// other than `uploaded`. A tarball of a few megabytes uploads in seconds, so a
/// minute covers a run that lost the race while the winner was still sending.
const UPLOAD_SETTLE_DELAYS: [Duration; 12] = [Duration::from_secs(5); 12];

/// The Release for `tag`, draft or published, that an upload or a checksum
/// reads.
async fn upload_target(crab: &Octocrab, repo: &Repo, tag: &str) -> Result<ForgeRelease> {
    find_release(crab, repo, tag)
        .await?
        .with_context(|| format!("release {tag} does not exist"))
}

async fn upload_release_asset_with(
    crab: &Octocrab,
    repo: &Repo,
    tag: &str,
    name: &str,
    body: bytes::Bytes,
    settle: &[Duration],
) -> Result<String> {
    let release = upload_target(crab, repo, tag).await?;
    if release.uploaded().contains(&name) {
        return Ok(format!("already {name}"));
    }
    let uploaded = crab
        .repos(&repo.owner, &repo.name)
        .releases()
        .upload_asset(release.id, name, body)
        .send()
        .await;
    match uploaded {
        Ok(asset) => Ok(asset.browser_download_url.to_string()),
        // Two runs can release the same commit, and both build every asset.
        // The tarballs differ in bytes (tar records file times), so a digest
        // cannot separate a duplicate from a mismatch. The name carries the
        // version and the target, and the tag fixes the commit, so a complete
        // asset of that name is this asset.
        Err(error) if is_already_exists(&error) => {
            let mut delays = settle.iter();
            loop {
                let release = upload_target(crab, repo, tag).await?;
                let asset = release
                    .assets
                    .iter()
                    .find(|asset| asset.name == name)
                    .with_context(|| {
                        format!(
                            "GitHub reports asset {name} exists, but release {tag} does not list it"
                        )
                    })?;
                if asset.state == "uploaded" {
                    return Ok(format!("already {name}"));
                }
                let Some(delay) = delays.next() else {
                    bail!(
                        "release {tag} has asset {name} in state {:?}, not uploaded, after waiting for another upload to finish; delete the asset and rerun",
                        asset.state
                    );
                };
                time::sleep(*delay).await;
            }
        }
        Err(error) => Err(error).context("upload release asset"),
    }
}

/// GitHub's 422 for an upload whose name the release already has, from
/// [upload a release asset].
/// The code is only in the error's body: octocrab displays a GitHub error as
/// "GitHub", so its text never names it.
///
/// [upload a release asset]: https://docs.github.com/en/rest/releases/assets#upload-a-release-asset
fn is_already_exists(error: &octocrab::Error) -> bool {
    matches!(error, octocrab::Error::GitHub { source, .. }
    if source.status_code.as_u16() == 422
        && source.errors.iter().flatten().any(|detail| {
            detail.get("code").and_then(serde_json::Value::as_str) == Some("already_exists")
        }))
}

/// Who opened a pull request, and where its head branch lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullAuthor {
    /// The author's login.
    pub login: String,
    /// GitHub's user `type`: `User` for a person, `Bot` for an app.
    pub kind: String,
    /// The head branch lives outside the repository: a fork, or a branch that
    /// GitHub has dropped. Maintainers push branches to the repository itself.
    pub from_fork: bool,
}

/// The author of pull request `number`, from
/// `GET /repos/{owner}/{repo}/pulls/{number}` ([get a pull request]).
///
/// # Errors
///
/// Returns an error when the GitHub client cannot be built or the request or
/// its decoding fails, for example when pull request `number` does not exist.
///
/// [get a pull request]: https://docs.github.com/en/rest/pulls/pulls#get-a-pull-request
pub fn pull_author(token: &str, repo: &Repo, number: u64) -> Result<PullAuthor> {
    block(async {
        let crab = client(token)?;
        pull_author_with(&crab, repo, number).await
    })
}

async fn pull_author_with(crab: &Octocrab, repo: &Repo, number: u64) -> Result<PullAuthor> {
    #[derive(serde::Deserialize)]
    struct User {
        login: String,
        #[serde(rename = "type")]
        kind: String,
    }
    #[derive(serde::Deserialize)]
    struct HeadRepo {
        full_name: String,
    }
    #[derive(serde::Deserialize)]
    struct Head {
        repo: Option<HeadRepo>,
    }
    #[derive(serde::Deserialize)]
    struct Pull {
        user: User,
        head: Head,
    }
    let route = format!("/repos/{}/{}/pulls/{number}", repo.owner, repo.name);
    let pull: Pull = crab
        .get(route, None::<&()>)
        .await
        .with_context(|| format!("read pull request #{number}"))?;
    let own = format!("{}/{}", repo.owner, repo.name);
    Ok(PullAuthor {
        login: pull.user.login,
        kind: pull.user.kind,
        from_fork: pull
            .head
            .repo
            .is_none_or(|head| !head.full_name.eq_ignore_ascii_case(&own)),
    })
}

/// A GitHub client for `token`.
/// It uses the API from `GITHUB_API_URL` when that is set, as on a GitHub
/// Enterprise Server or a test's mock server.
/// Otherwise it uses api.github.com.
fn client(token: &str) -> Result<Octocrab> {
    let mut builder = Octocrab::builder().personal_token(token.to_owned());
    if let Ok(base) = env::var("GITHUB_API_URL")
        && !base.is_empty()
    {
        builder = builder.base_uri(base).context("GITHUB_API_URL")?;
    }
    builder.build().context("GitHub client")
}

/// Run one future to completion on a fresh current-thread runtime, so the
/// blocking command handlers can call async clients.
pub(crate) fn block<F, T>(fut: F) -> Result<T>
where
    F: Future<Output = Result<T>>,
{
    Builder::new_current_thread()
        .enable_all()
        .build()
        .context("tokio")?
        .block_on(fut)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use indoc::indoc;
    use octocrab::service::middleware::retry::RetryConfig;
    use wiremock::matchers::{body_partial_json, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::{Octocrab, parse_remote, parse_slug};
    use crate::stop::{Record, Stop};

    #[test]
    fn slug_and_remotes() {
        let expected = super::Repo {
            owner: "victor-software-house".into(),
            name: "verctl".into(),
        };
        assert_eq!(
            parse_slug("victor-software-house/verctl").unwrap(),
            expected
        );
        assert_eq!(
            parse_remote("https://github.com/victor-software-house/verctl.git").unwrap(),
            expected
        );
        assert_eq!(
            parse_remote("git@github.com:victor-software-house/verctl.git").unwrap(),
            expected
        );
        assert_eq!(
            parse_remote("ssh://git@github.com/victor-software-house/verctl.git").unwrap(),
            expected
        );
    }

    #[test]
    fn pull_request_number_reads_only_a_pull_request_event() {
        assert_eq!(
            super::pull_request_number(r#"{"pull_request":{"number":187,"labels":[]}}"#),
            Some(187)
        );
        assert_eq!(super::pull_request_number("{}"), None);
        assert_eq!(super::pull_request_number("not json"), None);
        assert_eq!(
            super::pull_request_number(r#"{"issue":{"number":187}}"#),
            None
        );
    }

    #[test]
    fn is_merge_group_reads_only_a_merge_group_event() {
        assert!(super::is_merge_group(
            r#"{"action":"checks_requested","merge_group":{"head_ref":"refs/heads/gh-readonly-queue/main/pr-7-0000000000000000000000000000000000000001"}}"#
        ));
        assert!(!super::is_merge_group(r#"{"pull_request":{"number":7}}"#));
        assert!(!super::is_merge_group(r#"{"merge_group":null}"#));
        assert!(!super::is_merge_group("not json"));
    }

    #[test]
    fn default_branch_of_reads_the_event_repository() {
        assert_eq!(
            super::default_branch_of(
                r#"{"pull_request":{"number":1},"repository":{"default_branch":"main"}}"#
            ),
            Some("main".to_owned())
        );
        assert_eq!(
            super::default_branch_of(r#"{"repository":{"default_branch":""}}"#),
            None
        );
        assert_eq!(super::default_branch_of("{}"), None);
        assert_eq!(super::default_branch_of("not json"), None);
    }

    async fn pull_author_from(head: serde_json::Value) -> super::PullAuthor {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(format!("/repos/{OWNER}/{NAME}/pulls/12")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "number": 12,
                "user": { "login": "octo", "type": "User" },
                "head": head
            })))
            .mount(&server)
            .await;
        super::pull_author_with(&crab(&server.uri()), &repo(), 12)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn a_fork_head_is_a_fork() {
        let author =
            pull_author_from(serde_json::json!({ "repo": { "full_name": "octo/app" } })).await;
        assert_eq!(author.login, "octo");
        assert_eq!(author.kind, "User");
        assert!(author.from_fork);
    }

    #[tokio::test]
    async fn a_head_in_the_repository_is_not_a_fork() {
        let author = pull_author_from(serde_json::json!({
            "repo": { "full_name": format!("{OWNER}/{NAME}") }
        }))
        .await;
        assert!(!author.from_fork);
    }

    #[tokio::test]
    async fn a_deleted_fork_is_a_fork() {
        let author = pull_author_from(serde_json::json!({ "repo": null })).await;
        assert!(author.from_fork);
    }

    #[test]
    fn rejects_non_github() {
        assert!(parse_remote("https://gitlab.com/acme/app.git").is_err());
    }

    const OWNER: &str = "acme";
    const NAME: &str = "app";
    const TAG: &str = "v1.0.0";
    const PUBLISHED: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const OTHER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const TAG_OBJECT: &str = "cccccccccccccccccccccccccccccccccccccccc";

    fn repo() -> super::Repo {
        super::Repo {
            owner: OWNER.into(),
            name: NAME.into(),
        }
    }

    fn crab(uri: &str) -> Octocrab {
        Octocrab::builder()
            .personal_token("token".to_owned())
            .base_uri(uri)
            .unwrap()
            .build()
            .unwrap()
    }

    fn missing_release() -> Mock {
        Mock::given(method("GET"))
            .and(path(format!("/repos/{OWNER}/{NAME}/releases/tags/{TAG}")))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "message": "Not Found",
                "documentation_url": "https://docs.github.com"
            })))
    }

    /// `GET /releases`, the only listing that shows drafts. The lookup reads
    /// it when `GET /releases/tags/{tag}` answers 404.
    fn listed_releases(listed: serde_json::Value) -> Mock {
        Mock::given(method("GET"))
            .and(path(format!("/repos/{OWNER}/{NAME}/releases")))
            .respond_with(ResponseTemplate::new(200).set_body_json(listed))
    }

    fn no_releases_listed() -> Mock {
        listed_releases(serde_json::json!([]))
    }

    fn existing_release() -> Mock {
        Mock::given(method("GET"))
            .and(path(format!("/repos/{OWNER}/{NAME}/releases/tags/{TAG}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(release_json("main")))
    }

    /// A draft that holds `assets`, each a name and the state GitHub reports.
    fn draft_json(assets: &[(&str, &str)]) -> serde_json::Value {
        let mut json = release_json(PUBLISHED);
        json["draft"] = true.into();
        json["html_url"] =
            format!("https://github.com/{OWNER}/{NAME}/releases/tag/untagged-1").into();
        json["assets"] = assets
            .iter()
            .map(|(name, state)| serde_json::json!({ "name": name, "state": state }))
            .collect();
        json
    }

    fn declared() -> Vec<String> {
        vec![
            "app.pkg".to_owned(),
            "app_1.0.0_linux_x64.tar.gz".to_owned(),
        ]
    }

    /// `stable()` with the assets `declared()` names.
    fn with_assets(declared: &[String]) -> super::NewRelease<'_> {
        super::NewRelease {
            assets: declared,
            ..stable()
        }
    }

    fn create_tag_ref() -> Mock {
        Mock::given(method("POST"))
            .and(path(format!("/repos/{OWNER}/{NAME}/git/refs")))
            .and(body_partial_json(serde_json::json!({
                "ref": format!("refs/tags/{TAG}"),
                "sha": PUBLISHED
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "ref": format!("refs/tags/{TAG}"),
                "node_id": "R",
                "url": format!("https://api.github.com/repos/{OWNER}/{NAME}/git/refs/tags/{TAG}"),
                "object": {
                    "type": "commit",
                    "sha": PUBLISHED,
                    "url": format!("https://api.github.com/repos/{OWNER}/{NAME}/git/commits/{PUBLISHED}")
                }
            })))
    }

    fn create_draft() -> Mock {
        Mock::given(method("POST"))
            .and(path(format!("/repos/{OWNER}/{NAME}/releases")))
            .and(body_partial_json(serde_json::json!({
                "tag_name": TAG,
                "draft": true,
                "target_commitish": PUBLISHED
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(draft_json(&[])))
    }

    fn publish_draft() -> wiremock::MockBuilder {
        Mock::given(method("PATCH"))
            .and(path(format!("/repos/{OWNER}/{NAME}/releases/1")))
            .and(body_partial_json(serde_json::json!({ "draft": false })))
    }

    fn create_at_published_sha() -> Mock {
        Mock::given(method("POST"))
            .and(path(format!("/repos/{OWNER}/{NAME}/releases")))
            .and(body_partial_json(serde_json::json!({
                "tag_name": TAG,
                "target_commitish": PUBLISHED
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(release_json("main")))
    }

    fn stable() -> super::NewRelease<'static> {
        super::NewRelease {
            tag: TAG,
            name: TAG,
            body: "notes",
            sha: PUBLISHED,
            stage: super::Stage::Stable,
            assets: &[],
        }
    }

    fn lightweight_tag(sha: &str) -> Mock {
        Mock::given(method("GET"))
            .and(path(format!("/repos/{OWNER}/{NAME}/git/ref/tags/{TAG}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "ref": format!("refs/tags/{TAG}"),
                "node_id": "R",
                "url": format!("https://api.github.com/repos/{OWNER}/{NAME}/git/refs/tags/{TAG}"),
                "object": {
                    "type": "commit",
                    "sha": sha,
                    "url": format!("https://api.github.com/repos/{OWNER}/{NAME}/git/commits/{sha}")
                }
            })))
    }

    fn annotated_tag() -> Mock {
        Mock::given(method("GET"))
            .and(path(format!("/repos/{OWNER}/{NAME}/git/ref/tags/{TAG}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "ref": format!("refs/tags/{TAG}"),
                "node_id": "R",
                "url": format!("https://api.github.com/repos/{OWNER}/{NAME}/git/refs/tags/{TAG}"),
                "object": {
                    "type": "tag",
                    "sha": TAG_OBJECT,
                    "url": format!("https://api.github.com/repos/{OWNER}/{NAME}/git/tags/{TAG_OBJECT}")
                }
            })))
    }

    fn peeled_tag_object(sha: &str) -> Mock {
        Mock::given(method("GET"))
            .and(path(format!("/repos/{OWNER}/{NAME}/git/tags/{TAG_OBJECT}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "node_id": "T",
                "tag": TAG,
                "sha": TAG_OBJECT,
                "url": format!("https://api.github.com/repos/{OWNER}/{NAME}/git/tags/{TAG_OBJECT}"),
                "message": TAG,
                "object": {
                    "type": "commit",
                    "sha": sha,
                    "url": format!("https://api.github.com/repos/{OWNER}/{NAME}/git/commits/{sha}")
                }
            })))
    }

    fn release_json(target_commitish: &str) -> serde_json::Value {
        serde_json::json!({
            "url": format!("https://api.github.com/repos/{OWNER}/{NAME}/releases/1"),
            "html_url": format!("https://github.com/{OWNER}/{NAME}/releases/tag/{TAG}"),
            "assets_url": format!("https://api.github.com/repos/{OWNER}/{NAME}/releases/1/assets"),
            "upload_url": format!("https://uploads.github.com/repos/{OWNER}/{NAME}/releases/1/assets{{?name,label}}"),
            "tarball_url": format!("https://api.github.com/repos/{OWNER}/{NAME}/tarball/{TAG}"),
            "zipball_url": format!("https://api.github.com/repos/{OWNER}/{NAME}/zipball/{TAG}"),
            "id": 1,
            "node_id": "R",
            "tag_name": TAG,
            "target_commitish": target_commitish,
            "name": TAG,
            "body": "notes",
            "draft": false,
            "prerelease": false,
            "created_at": "2026-08-22T00:00:00Z",
            "published_at": "2026-08-22T00:00:00Z",
            "assets": []
        })
    }

    fn release_with_assets(assets: &[(&str, &str)]) -> Mock {
        let mut json = release_json("main");
        json["assets"] = assets
            .iter()
            .map(|(name, state)| serde_json::json!({ "name": name, "state": state }))
            .collect();
        Mock::given(method("GET"))
            .and(path(format!("/repos/{OWNER}/{NAME}/releases/tags/{TAG}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json))
    }

    fn missing_tag() -> Mock {
        Mock::given(method("GET"))
            .and(path(format!("/repos/{OWNER}/{NAME}/git/ref/tags/{TAG}")))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "message": "Not Found",
                "documentation_url": "https://docs.github.com"
            })))
    }

    #[tokio::test]
    async fn a_release_never_made_has_no_tag_and_no_release() {
        let server = MockServer::start().await;
        missing_tag().mount(&server).await;
        missing_release().expect(1).mount(&server).await;
        no_releases_listed().expect(1).mount(&server).await;
        let state = super::release_state_with(&crab(&server.uri()), &repo(), TAG)
            .await
            .unwrap();
        assert_eq!(state, super::ReleaseState::default());
    }

    #[tokio::test]
    async fn a_finished_release_names_its_commit_and_assets() {
        let server = MockServer::start().await;
        lightweight_tag(PUBLISHED).mount(&server).await;
        release_with_assets(&[
            ("app-darwin-arm64.tar.gz", "uploaded"),
            ("app-linux-x64.tar.gz", "uploaded"),
            ("app.pkg", "starter"),
        ])
        .mount(&server)
        .await;
        let state = super::release_state_with(&crab(&server.uri()), &repo(), TAG)
            .await
            .unwrap();
        assert_eq!(state.tag.as_deref(), Some(PUBLISHED));
        assert!(state.release);
        assert!(!state.draft);
        assert_eq!(
            state.assets,
            ["app-darwin-arm64.tar.gz", "app-linux-x64.tar.gz"]
        );
    }

    #[tokio::test]
    async fn an_immutable_release_is_read_as_immutable() {
        let server = MockServer::start().await;
        lightweight_tag(PUBLISHED).mount(&server).await;
        let mut json = release_json("main");
        json["immutable"] = true.into();
        Mock::given(method("GET"))
            .and(path(format!("/repos/{OWNER}/{NAME}/releases/tags/{TAG}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json))
            .mount(&server)
            .await;
        let state = super::release_state_with(&crab(&server.uri()), &repo(), TAG)
            .await
            .unwrap();
        assert!(
            state.release && !state.draft && state.immutable,
            "{state:?}"
        );
    }

    /// `GET /releases/tags/{tag}` answers 404 for a draft, so the lookup reads
    /// the list, and only an asset in state `uploaded` counts.
    #[tokio::test]
    async fn a_draft_is_found_through_the_release_list() {
        let server = MockServer::start().await;
        lightweight_tag(PUBLISHED).mount(&server).await;
        missing_release().mount(&server).await;
        let mut other = release_json("main");
        other["tag_name"] = "v0.9.0".into();
        listed_releases(serde_json::json!([
            other,
            draft_json(&[
                ("app.pkg", "uploaded"),
                ("app_1.0.0_linux_x64.tar.gz", "starter")
            ])
        ]))
        .mount(&server)
        .await;
        let state = super::release_state_with(&crab(&server.uri()), &repo(), TAG)
            .await
            .unwrap();
        assert!(
            state.release && state.draft && !state.immutable,
            "{state:?}"
        );
        assert_eq!(state.assets, ["app.pkg"]);
    }

    /// A tag with no Release opens a draft at HEAD, and no tag is created:
    /// GitHub creates it when the draft is published.
    #[tokio::test]
    async fn a_missing_release_opens_a_draft_at_head_with_no_tag() {
        let server = MockServer::start().await;
        create_tag_ref().expect(0).mount(&server).await;
        create_draft().expect(1).mount(&server).await;
        let declared = declared();
        let id = super::open_draft_with(
            &crab(&server.uri()),
            &repo(),
            &with_assets(&declared),
            &super::Unpublished::Missing,
        )
        .await
        .unwrap();
        assert_eq!(id, 1);
    }

    #[tokio::test]
    async fn a_release_with_no_assets_is_published_at_once() {
        let server = MockServer::start().await;
        missing_release().mount(&server).await;
        no_releases_listed().mount(&server).await;
        lightweight_tag(PUBLISHED).mount(&server).await;
        Mock::given(method("POST"))
            .and(path(format!("/repos/{OWNER}/{NAME}/releases")))
            .and(body_partial_json(serde_json::json!({
                "tag_name": TAG,
                "target_commitish": PUBLISHED
            })))
            .and(|request: &wiremock::Request| {
                let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
                body.get("draft").is_none()
            })
            .respond_with(ResponseTemplate::new(201).set_body_json(release_json("main")))
            .expect(1)
            .mount(&server)
            .await;
        create_tag_ref().expect(0).mount(&server).await;
        let url = super::ensure_release_with(&crab(&server.uri()), &repo(), &stable(), true)
            .await
            .unwrap();
        assert!(url.contains(TAG), "{url}");
    }

    /// The draft as `GET /releases/{id}` returns it, holding `assets`.
    fn draft_by_id(assets: &[(&str, &str)]) -> Mock {
        Mock::given(method("GET"))
            .and(path(format!("/repos/{OWNER}/{NAME}/releases/1")))
            .respond_with(ResponseTemplate::new(200).set_body_json(draft_json(assets)))
    }

    /// The update sends this run's title, notes and commit, so a draft
    /// someone edited reads as this run would have written it.
    #[tokio::test]
    async fn a_draft_with_every_asset_uploaded_is_published() {
        let server = MockServer::start().await;
        draft_by_id(&[
            ("app.pkg", "uploaded"),
            ("app_1.0.0_linux_x64.tar.gz", "uploaded"),
        ])
        .mount(&server)
        .await;
        publish_draft()
            .and(body_partial_json(serde_json::json!({
                "name": TAG,
                "body": "notes",
                "target_commitish": PUBLISHED,
                "make_latest": "legacy"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(release_json(PUBLISHED)))
            .expect(1)
            .mount(&server)
            .await;
        let declared = declared();
        let url =
            super::publish_draft_with(&crab(&server.uri()), &repo(), &with_assets(&declared), 1)
                .await
                .unwrap();
        assert_eq!(
            url,
            format!("https://github.com/{OWNER}/{NAME}/releases/tag/{TAG}")
        );
    }

    #[tokio::test]
    async fn a_candidate_draft_is_published_as_a_prerelease_that_is_never_the_latest() {
        let server = MockServer::start().await;
        draft_by_id(&[("app.pkg", "uploaded")]).mount(&server).await;
        Mock::given(method("PATCH"))
            .and(path(format!("/repos/{OWNER}/{NAME}/releases/1")))
            .and(body_partial_json(serde_json::json!({
                "draft": false,
                "prerelease": true,
                "make_latest": "false"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(release_json(PUBLISHED)))
            .expect(1)
            .mount(&server)
            .await;
        let declared = ["app.pkg".to_owned()];
        let candidate = super::NewRelease {
            stage: super::Stage::Candidate,
            ..with_assets(&declared)
        };
        super::publish_draft_with(&crab(&server.uri()), &repo(), &candidate, 1)
            .await
            .unwrap();
    }

    /// Only an asset in state `uploaded` counts, and a draft that lacks one is
    /// never published.
    #[tokio::test]
    async fn a_draft_missing_an_asset_is_not_published_and_names_what_it_lacks() {
        let server = MockServer::start().await;
        draft_by_id(&[
            ("app.pkg", "uploaded"),
            ("app_1.0.0_linux_x64.tar.gz", "starter"),
        ])
        .mount(&server)
        .await;
        publish_draft()
            .respond_with(ResponseTemplate::new(500))
            .expect(0)
            .mount(&server)
            .await;
        let declared = declared();
        let error =
            super::publish_draft_with(&crab(&server.uri()), &repo(), &with_assets(&declared), 1)
                .await
                .unwrap_err();
        let message = format!("{error:#}");
        assert!(
            message.contains(&format!("{TAG} lacks app_1.0.0_linux_x64.tar.gz")),
            "{message}"
        );
    }

    async fn found(server: &MockServer) -> anyhow::Result<super::Found> {
        super::read_release_with(&crab(&server.uri()), &repo(), TAG, PUBLISHED).await
    }

    /// A tag on another commit fails before any write, whatever its Release:
    /// publishing a draft keeps an existing tag where it is.
    #[tokio::test]
    async fn a_tag_on_another_commit_fails_before_any_write() {
        let server = MockServer::start().await;
        missing_release().mount(&server).await;
        lightweight_tag(OTHER).mount(&server).await;
        listed_releases(serde_json::json!([draft_json(&[])]))
            .mount(&server)
            .await;
        let message = format!("{:#}", found(&server).await.unwrap_err());
        assert!(message.contains(OTHER), "{message}");
        assert!(message.contains(PUBLISHED), "{message}");
    }

    #[tokio::test]
    async fn a_draft_at_head_is_kept() {
        let server = MockServer::start().await;
        missing_tag().mount(&server).await;
        missing_release().mount(&server).await;
        listed_releases(serde_json::json!([draft_json(&[("app.pkg", "uploaded")])]))
            .mount(&server)
            .await;
        let super::Found::Unpublished(draft @ super::Unpublished::AtHead(_)) =
            found(&server).await.unwrap()
        else {
            panic!("a draft at HEAD");
        };
        assert_eq!(draft.uploaded(), ["app.pkg"]);
    }

    /// A draft at a branch name, as one created with GitHub's default target,
    /// is stale, and none of its assets count.
    #[tokio::test]
    async fn a_draft_at_another_commit_is_stale() {
        let server = MockServer::start().await;
        missing_tag().mount(&server).await;
        missing_release().mount(&server).await;
        let mut stale = draft_json(&[("app.pkg", "uploaded")]);
        stale["target_commitish"] = "main".into();
        listed_releases(serde_json::json!([stale]))
            .mount(&server)
            .await;
        let super::Found::Unpublished(draft @ super::Unpublished::Stale(_)) =
            found(&server).await.unwrap()
        else {
            panic!("a stale draft");
        };
        assert_eq!(draft.uploaded(), Vec::<&str>::new());
    }

    /// GitHub can answer 404 for a tag a moment after the Release that created
    /// it is published. A Release published at HEAD then counts as at HEAD.
    #[tokio::test]
    async fn a_release_published_at_head_before_its_tag_is_served_counts_as_published() {
        let server = MockServer::start().await;
        missing_tag().mount(&server).await;
        Mock::given(method("GET"))
            .and(path(format!("/repos/{OWNER}/{NAME}/releases/tags/{TAG}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(release_json(PUBLISHED)))
            .mount(&server)
            .await;
        assert!(
            matches!(found(&server).await.unwrap(), super::Found::Published(_)),
            "published at HEAD"
        );
    }

    #[tokio::test]
    async fn a_release_published_elsewhere_with_no_tag_fails() {
        let server = MockServer::start().await;
        missing_tag().mount(&server).await;
        Mock::given(method("GET"))
            .and(path(format!("/repos/{OWNER}/{NAME}/releases/tags/{TAG}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(release_json(OTHER)))
            .mount(&server)
            .await;
        let message = format!("{:#}", found(&server).await.unwrap_err());
        assert!(message.contains(OTHER), "{message}");
        assert!(message.contains(PUBLISHED), "{message}");
    }

    /// A stale draft is deleted before the draft at HEAD is created.
    #[tokio::test]
    async fn a_stale_draft_is_deleted_and_created_again_at_head() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path(format!("/repos/{OWNER}/{NAME}/releases/7")))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        create_draft().expect(1).mount(&server).await;
        let mut stale = draft_json(&[]);
        stale["id"] = 7.into();
        stale["target_commitish"] = OTHER.into();
        let stale = serde_json::from_value(stale).unwrap();
        let id = super::open_draft_with(
            &crab(&server.uri()),
            &repo(),
            &stable(),
            &super::Unpublished::Stale(stale),
        )
        .await
        .unwrap();
        assert_eq!(id, 1);
        let methods: Vec<String> = server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .map(|request| request.method.to_string())
            .collect();
        assert_eq!(methods, ["DELETE", "POST"]);
    }

    /// A 403 on the draft names the permission, and the run stops there,
    /// before any upload.
    #[tokio::test]
    async fn a_forbidden_draft_names_the_workflows_permission() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(format!("/repos/{OWNER}/{NAME}/releases")))
            .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
                "message": "Resource not accessible by integration",
                "documentation_url": "https://docs.github.com/rest/releases/releases#create-a-release"
            })))
            .mount(&server)
            .await;
        let error = super::open_draft_with(
            &crab(&server.uri()),
            &repo(),
            &stable(),
            &super::Unpublished::Missing,
        )
        .await
        .unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("workflows: write"), "{message}");
        assert!(
            error.downcast_ref::<super::ReleaseRefused>().is_some(),
            "{message}"
        );
    }

    #[tokio::test]
    async fn a_draft_at_head_is_used_as_it_is() {
        let server = MockServer::start().await;
        let draft = serde_json::from_value(draft_json(&[])).unwrap();
        let id = super::open_draft_with(
            &crab(&server.uri()),
            &repo(),
            &stable(),
            &super::Unpublished::AtHead(draft),
        )
        .await
        .unwrap();
        assert_eq!(id, 1);
        assert_eq!(server.received_requests().await.unwrap().len(), 0);
    }

    const NO_WAIT: [Duration; 4] = [Duration::ZERO; 4];

    async fn reads_of_the_tag(server: &MockServer) -> usize {
        server.received_requests().await.unwrap().len()
    }

    #[tokio::test]
    async fn the_tag_proof_reads_again_after_a_404() {
        let server = MockServer::start().await;
        missing_tag().up_to_n_times(1).mount(&server).await;
        lightweight_tag(PUBLISHED).mount(&server).await;
        super::prove_tag_with(&crab(&server.uri()), &repo(), TAG, PUBLISHED, &NO_WAIT)
            .await
            .unwrap();
        assert_eq!(reads_of_the_tag(&server).await, 2);
    }

    #[tokio::test]
    async fn the_tag_proof_fails_after_five_404s() {
        let server = MockServer::start().await;
        missing_tag().mount(&server).await;
        let error = super::prove_tag_with(&crab(&server.uri()), &repo(), TAG, PUBLISHED, &NO_WAIT)
            .await
            .unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains(&format!("no tag {TAG}")), "{message}");
        assert!(message.contains("after 5 reads"), "{message}");
        assert_eq!(reads_of_the_tag(&server).await, 5);
    }

    /// Any error but a 404 fails at once. octocrab retries a 5xx itself, so
    /// the client here makes one request per read.
    #[tokio::test]
    async fn the_tag_proof_fails_at_once_on_any_other_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(format!("/repos/{OWNER}/{NAME}/git/ref/tags/{TAG}")))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let once = Octocrab::builder()
            .personal_token("token".to_owned())
            .base_uri(server.uri())
            .unwrap()
            .add_retry_config(RetryConfig::None)
            .build()
            .unwrap();
        let error = super::prove_tag_with(&once, &repo(), TAG, PUBLISHED, &NO_WAIT)
            .await
            .unwrap_err();
        assert!(
            format!("{error:#}").contains(&format!("read tag {TAG}")),
            "{error:#}"
        );
        assert_eq!(reads_of_the_tag(&server).await, 1);
    }

    #[tokio::test]
    async fn the_tag_proof_fails_on_a_tag_at_another_commit() {
        let server = MockServer::start().await;
        lightweight_tag(OTHER).mount(&server).await;
        let error = super::prove_tag_with(&crab(&server.uri()), &repo(), TAG, PUBLISHED, &NO_WAIT)
            .await
            .unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains(OTHER), "{message}");
        assert!(message.contains(PUBLISHED), "{message}");
    }

    fn other_releases(count: usize) -> Vec<serde_json::Value> {
        (0..count)
            .map(|n| {
                let mut json = release_json("main");
                json["tag_name"] = format!("v0.0.{n}").into();
                json
            })
            .collect()
    }

    /// The list is read a page at a time, 100 to a page, until a Release has
    /// the tag. Each mock answers only its own page.
    #[tokio::test]
    async fn the_release_list_is_read_page_by_page_until_the_tag_is_found() {
        let server = MockServer::start().await;
        missing_release().mount(&server).await;
        Mock::given(method("GET"))
            .and(path(format!("/repos/{OWNER}/{NAME}/releases")))
            .and(query_param("per_page", "100"))
            .and(query_param("page", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(other_releases(100)))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("/repos/{OWNER}/{NAME}/releases")))
            .and(query_param("per_page", "100"))
            .and(query_param("page", "2"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!([draft_json(&[])])),
            )
            .expect(1)
            .mount(&server)
            .await;
        let found = super::find_release(&crab(&server.uri()), &repo(), TAG)
            .await
            .unwrap()
            .expect("the draft on page 2");
        assert!(found.draft);
    }

    /// A draft has no tag until it is published, so a missing tag still reads
    /// the Releases, and a tagless draft is an incomplete release.
    #[tokio::test]
    async fn a_tagless_draft_is_an_incomplete_release() {
        let server = MockServer::start().await;
        missing_tag().mount(&server).await;
        missing_release().mount(&server).await;
        listed_releases(serde_json::json!([draft_json(&[("app.pkg", "uploaded")])]))
            .mount(&server)
            .await;
        let state = super::release_state_with(&crab(&server.uri()), &repo(), TAG)
            .await
            .unwrap();
        assert_eq!(state.tag, None);
        assert!(state.release && state.draft, "{state:?}");
        assert_eq!(state.assets, ["app.pkg"]);
    }

    /// A draft's stop record is read from its body, for `status` to show.
    #[tokio::test]
    async fn a_drafts_stop_record_is_read() {
        let server = MockServer::start().await;
        missing_tag().mount(&server).await;
        missing_release().mount(&server).await;
        let mut draft = draft_json(&[]);
        draft["body"] = indoc! {r#"
            notes

            <!-- verctl:stop {"package":"app","version":"1.0.0","cause":"429 Too Many Requests"} -->
        "#}
        .into();
        listed_releases(serde_json::json!([draft]))
            .mount(&server)
            .await;
        let state = super::release_state_with(&crab(&server.uri()), &repo(), TAG)
            .await
            .unwrap();
        assert_eq!(
            state.stop,
            Record::Read(Stop::new("app", "1.0.0", "429 Too Many Requests", None))
        );
    }

    /// A tag with no Release in a repository with many reads ten pages, then
    /// counts as having none, instead of one request per page forever.
    #[tokio::test]
    async fn the_release_list_is_read_for_ten_pages_at_most() {
        let server = MockServer::start().await;
        missing_release().mount(&server).await;
        Mock::given(method("GET"))
            .and(path(format!("/repos/{OWNER}/{NAME}/releases")))
            .respond_with(ResponseTemplate::new(200).set_body_json(other_releases(100)))
            .expect(10)
            .mount(&server)
            .await;
        let found = super::find_release(&crab(&server.uri()), &repo(), TAG)
            .await
            .unwrap();
        assert!(found.is_none(), "{found:?}");
    }

    #[tokio::test]
    async fn create_tags_the_published_commit_not_the_default_branch() {
        let server = MockServer::start().await;
        missing_release().mount(&server).await;
        no_releases_listed().mount(&server).await;
        create_at_published_sha().mount(&server).await;
        lightweight_tag(PUBLISHED).mount(&server).await;
        let url = super::ensure_release_with(&crab(&server.uri()), &repo(), &stable(), true)
            .await
            .unwrap();
        assert!(url.contains(TAG), "{url}");
    }

    #[tokio::test]
    async fn a_candidate_release_is_a_prerelease_that_is_never_the_latest() {
        let server = MockServer::start().await;
        missing_release().mount(&server).await;
        no_releases_listed().mount(&server).await;
        Mock::given(method("POST"))
            .and(path(format!("/repos/{OWNER}/{NAME}/releases")))
            .and(body_partial_json(serde_json::json!({
                "tag_name": TAG,
                "prerelease": true,
                "make_latest": "false"
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(release_json("main")))
            .mount(&server)
            .await;
        lightweight_tag(PUBLISHED).mount(&server).await;
        let candidate = super::NewRelease {
            stage: super::Stage::Candidate,
            ..stable()
        };
        let url = super::ensure_release_with(&crab(&server.uri()), &repo(), &candidate, true)
            .await
            .unwrap();
        assert!(url.contains(TAG), "{url}");
    }

    #[tokio::test]
    async fn a_stable_release_sets_neither_prerelease_nor_latest() {
        let server = MockServer::start().await;
        missing_release().mount(&server).await;
        no_releases_listed().mount(&server).await;
        Mock::given(method("POST"))
            .and(path(format!("/repos/{OWNER}/{NAME}/releases")))
            .and(|request: &wiremock::Request| {
                let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
                body.get("prerelease").is_none() && body.get("make_latest").is_none()
            })
            .respond_with(ResponseTemplate::new(201).set_body_json(release_json("main")))
            .mount(&server)
            .await;
        lightweight_tag(PUBLISHED).mount(&server).await;
        let url = super::ensure_release_with(&crab(&server.uri()), &repo(), &stable(), true)
            .await
            .unwrap();
        assert!(url.contains(TAG), "{url}");
    }

    #[tokio::test]
    async fn a_forbidden_release_names_the_workflows_permission() {
        let server = MockServer::start().await;
        missing_release().mount(&server).await;
        no_releases_listed().mount(&server).await;
        Mock::given(method("POST"))
            .and(path(format!("/repos/{OWNER}/{NAME}/releases")))
            .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
                "message": "Resource not accessible by integration",
                "documentation_url": "https://docs.github.com/rest/releases/releases#create-a-release"
            })))
            .mount(&server)
            .await;
        let error = super::ensure_release_with(&crab(&server.uri()), &repo(), &stable(), true)
            .await
            .unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("workflows: write"), "{message}");
        assert!(message.contains(PUBLISHED), "{message}");
        assert!(
            error.downcast_ref::<super::ReleaseRefused>().is_some(),
            "{message}"
        );
    }

    #[tokio::test]
    async fn create_fails_when_the_tag_lands_elsewhere() {
        let server = MockServer::start().await;
        missing_release().mount(&server).await;
        no_releases_listed().mount(&server).await;
        create_at_published_sha().mount(&server).await;
        lightweight_tag(OTHER).mount(&server).await;
        let error = super::ensure_release_with(&crab(&server.uri()), &repo(), &stable(), true)
            .await
            .unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains(OTHER), "{message}");
        assert!(message.contains(PUBLISHED), "{message}");
    }

    #[tokio::test]
    async fn existing_release_at_the_published_commit_is_ok() {
        let server = MockServer::start().await;
        existing_release().mount(&server).await;
        lightweight_tag(PUBLISHED).mount(&server).await;
        let url = super::ensure_release_with(&crab(&server.uri()), &repo(), &stable(), true)
            .await
            .unwrap();
        assert!(url.contains(TAG), "{url}");
    }

    #[tokio::test]
    async fn existing_release_at_another_commit_fails() {
        let server = MockServer::start().await;
        existing_release().mount(&server).await;
        lightweight_tag(OTHER).mount(&server).await;
        let error = super::ensure_release_with(&crab(&server.uri()), &repo(), &stable(), true)
            .await
            .unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains(OTHER), "{message}");
        assert!(message.contains(PUBLISHED), "{message}");
    }

    #[tokio::test]
    async fn existing_release_is_kept_when_nothing_under_it_shipped() {
        let server = MockServer::start().await;
        existing_release().mount(&server).await;
        lightweight_tag(OTHER).mount(&server).await;
        let url = super::ensure_release_with(&crab(&server.uri()), &repo(), &stable(), false)
            .await
            .unwrap();
        assert!(url.contains(TAG), "{url}");
    }

    #[tokio::test]
    async fn annotated_tag_peels_to_the_commit() {
        let server = MockServer::start().await;
        existing_release().mount(&server).await;
        annotated_tag().mount(&server).await;
        peeled_tag_object(PUBLISHED).mount(&server).await;
        let url = super::ensure_release_with(&crab(&server.uri()), &repo(), &stable(), true)
            .await
            .unwrap();
        assert!(url.contains(TAG), "{url}");
    }

    fn upload_target_json(assets: &serde_json::Value) -> Mock {
        let mut json = release_json("main");
        json["assets"] = assets.clone();
        Mock::given(method("GET"))
            .and(path(format!("/repos/{OWNER}/{NAME}/releases/tags/{TAG}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json))
    }

    /// The release octocrab reads before an upload, pointing uploads at `uri`.
    fn release_by_id(uri: &str) -> Mock {
        let mut json = release_json("main");
        json["upload_url"] =
            format!("{uri}/upload/repos/{OWNER}/{NAME}/releases/1/assets{{?name,label}}").into();
        Mock::given(method("GET"))
            .and(path(format!("/repos/{OWNER}/{NAME}/releases/1")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json))
    }

    fn upload_refused(status: u16, body: serde_json::Value) -> Mock {
        Mock::given(method("POST"))
            .and(path(format!(
                "/upload/repos/{OWNER}/{NAME}/releases/1/assets"
            )))
            .respond_with(ResponseTemplate::new(status).set_body_json(body))
    }

    fn already_exists() -> serde_json::Value {
        serde_json::json!({
            "message": "Validation Failed",
            "errors": [{ "resource": "ReleaseAsset", "code": "already_exists", "field": "name" }],
            "documentation_url": "https://docs.github.com/rest/releases/assets#upload-a-release-asset"
        })
    }

    async fn upload(server: &MockServer) -> anyhow::Result<String> {
        super::upload_release_asset_with(
            &crab(&server.uri()),
            &repo(),
            TAG,
            "app_1.0.0_linux_x64.tar.gz",
            bytes::Bytes::from_static(b"tarball"),
            &[Duration::ZERO; 2],
        )
        .await
    }

    #[tokio::test]
    async fn an_asset_another_run_uploaded_is_already_done() {
        let server = MockServer::start().await;
        upload_target_json(&serde_json::json!([
            { "name": "app_1.0.0_linux_x64.tar.gz", "state": "uploaded" }
        ]))
        .mount(&server)
        .await;
        release_by_id(&server.uri()).mount(&server).await;
        upload_refused(422, already_exists())
            .expect(0)
            .mount(&server)
            .await;
        assert_eq!(
            upload(&server).await.unwrap(),
            "already app_1.0.0_linux_x64.tar.gz"
        );
    }

    #[tokio::test]
    async fn a_duplicate_waits_for_the_other_upload_to_finish() {
        let server = MockServer::start().await;
        // The read before the upload and the first read after the refusal see
        // the other run still sending.
        upload_target_json(&serde_json::json!([
            { "name": "app_1.0.0_linux_x64.tar.gz", "state": "starter" }
        ]))
        .up_to_n_times(2)
        .mount(&server)
        .await;
        upload_target_json(&serde_json::json!([
            { "name": "app_1.0.0_linux_x64.tar.gz", "state": "uploaded" }
        ]))
        .mount(&server)
        .await;
        release_by_id(&server.uri()).mount(&server).await;
        upload_refused(422, already_exists()).mount(&server).await;
        assert_eq!(
            upload(&server).await.unwrap(),
            "already app_1.0.0_linux_x64.tar.gz"
        );
    }

    #[tokio::test]
    async fn an_asset_whose_upload_broke_off_fails() {
        let server = MockServer::start().await;
        upload_target_json(&serde_json::json!([
            { "name": "app_1.0.0_linux_x64.tar.gz", "state": "starter" }
        ]))
        .mount(&server)
        .await;
        release_by_id(&server.uri()).mount(&server).await;
        upload_refused(422, already_exists()).mount(&server).await;
        let error = upload(&server).await.unwrap_err();
        assert!(format!("{error:#}").contains("\"starter\""), "{error:#}");
    }

    #[tokio::test]
    async fn another_validation_failure_fails_the_upload() {
        let server = MockServer::start().await;
        upload_target_json(&serde_json::json!([]))
            .mount(&server)
            .await;
        release_by_id(&server.uri()).mount(&server).await;
        upload_refused(
            422,
            serde_json::json!({
                "message": "Validation Failed",
                "errors": [{ "resource": "ReleaseAsset", "code": "invalid", "field": "name" }]
            }),
        )
        .mount(&server)
        .await;
        let error = upload(&server).await.unwrap_err();
        assert!(
            format!("{error:#}").contains("upload release asset"),
            "{error:#}"
        );
    }

    fn create_vendored_release() -> wiremock::MockBuilder {
        Mock::given(method("POST"))
            .and(path(format!("/repos/{OWNER}/{NAME}/releases")))
            .and(body_partial_json(serde_json::json!({
                "tag_name": TAG,
                "name": TAG,
                "body": "Built from upstream.",
                "target_commitish": PUBLISHED,
                "make_latest": "false"
            })))
    }

    async fn vendored_release(server: &MockServer) -> anyhow::Result<String> {
        super::ensure_vendored_release_with(
            &crab(&server.uri()),
            &repo(),
            TAG,
            "Built from upstream.",
            PUBLISHED,
        )
        .await
    }

    #[tokio::test]
    async fn a_missing_vendored_release_is_created_and_not_made_latest() {
        let server = MockServer::start().await;
        missing_release().mount(&server).await;
        create_vendored_release()
            .respond_with(ResponseTemplate::new(201).set_body_json(release_json(PUBLISHED)))
            .expect(1)
            .mount(&server)
            .await;
        let url = vendored_release(&server).await.unwrap();
        assert!(url.ends_with(&format!("/releases/tag/{TAG}")), "{url}");
    }

    #[tokio::test]
    async fn an_existing_vendored_release_is_used_wherever_its_tag_points() {
        let server = MockServer::start().await;
        existing_release().mount(&server).await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500))
            .expect(0)
            .mount(&server)
            .await;
        let url = vendored_release(&server).await.unwrap();
        assert!(url.contains(TAG), "{url}");
    }

    #[tokio::test]
    async fn a_vendored_release_another_job_created_first_is_used() {
        let server = MockServer::start().await;
        missing_release().up_to_n_times(1).mount(&server).await;
        existing_release().mount(&server).await;
        create_vendored_release()
            .respond_with(ResponseTemplate::new(422).set_body_json(serde_json::json!({
                "message": "Validation Failed",
                "errors": [{ "resource": "Release", "code": "already_exists", "field": "tag_name" }]
            })))
            .expect(1)
            .mount(&server)
            .await;
        let url = vendored_release(&server).await.unwrap();
        assert!(url.contains(TAG), "{url}");
    }

    async fn sha256_of(server: &MockServer, name: &str) -> anyhow::Result<String> {
        super::asset_sha256_with(&crab(&server.uri()), &repo(), TAG, name).await
    }

    #[tokio::test]
    async fn the_checksum_is_the_digest_github_holds() {
        let server = MockServer::start().await;
        upload_target_json(&serde_json::json!([
            { "name": "lib-1.0.0-linux-x64.tar.gz", "state": "uploaded", "digest": "sha256:abc123" }
        ]))
        .mount(&server)
        .await;
        assert_eq!(
            sha256_of(&server, "lib-1.0.0-linux-x64.tar.gz")
                .await
                .unwrap(),
            "abc123"
        );
    }

    #[tokio::test]
    async fn an_asset_without_a_digest_has_no_checksum() {
        let server = MockServer::start().await;
        upload_target_json(&serde_json::json!([
            { "name": "lib-1.0.0-linux-x64.tar.gz", "state": "uploaded", "digest": null }
        ]))
        .mount(&server)
        .await;
        let error = sha256_of(&server, "lib-1.0.0-linux-x64.tar.gz")
            .await
            .unwrap_err();
        assert!(
            format!("{error:#}").contains("no SHA-256 digest"),
            "{error:#}"
        );
    }
}
