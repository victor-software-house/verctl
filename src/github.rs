use anyhow::{Context, Result, bail, ensure};
use octocrab::Octocrab;
use octocrab::params::State;
use std::env;
use std::path::Path;

/// Append to `$GITHUB_OUTPUT`. That file belongs to the whole step, so a
/// writer that truncates would drop assignments another step already made.
/// GitHub takes the last value for a repeated key.
/// A previous writer that skipped its own trailing newline would otherwise
/// have its last line fused onto this one, losing both assignments.
pub fn write_output(path: &Path, body: &str) -> Result<()> {
    use std::io::{Read, Seek, SeekFrom, Write};
    let mut file = std::fs::OpenOptions::new()
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    pub owner: String,
    pub name: String,
}

/// Actions sets `GITHUB_REPOSITORY`. Locally, `origin` is the contract.
pub fn repo(root: &Path) -> Result<Repo> {
    if let Ok(slug) = env::var("GITHUB_REPOSITORY")
        && !slug.is_empty()
    {
        return parse_slug(&slug);
    }
    let url = crate::git::origin_url(root)?;
    parse_remote(url.trim())
}

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

#[must_use]
pub fn base_branch(root: &Path) -> String {
    if let Ok(name) = env::var("GITHUB_REF_NAME")
        && !name.is_empty()
    {
        return name;
    }
    crate::git::upstream_default_branch(root).unwrap_or_else(|| "main".into())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExistingPr {
    pub url: String,
    pub number: u64,
}

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

/// Label GitHub recorded on this `pull_request` event. Not `GITHUB_HEAD_REF`.
#[must_use]
pub fn event_labels() -> Vec<String> {
    let Ok(path) = env::var("GITHUB_EVENT_PATH") else {
        return Vec::new();
    };
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    labels_from_event(&raw)
}

#[must_use]
pub fn event_has_label(name: &str) -> bool {
    event_labels().iter().any(|label| label == name)
}

#[must_use]
pub fn labels_from_event(raw: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return Vec::new();
    };
    value
        .get("pull_request")
        .and_then(|node| node.get("labels"))
        .and_then(|labels| labels.as_array())
        .map(|labels| {
            labels
                .iter()
                .filter_map(|label| label.get("name")?.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// Create the label if needed, then apply it to the Version PR.
pub fn ensure_version_label(token: &str, repo: &Repo, number: u64, label: &str) -> Result<()> {
    if label.is_empty() {
        return Ok(());
    }
    let label = label.to_owned();
    block(async {
        let crab = client(token)?;
        let issues = crab.issues(&repo.owner, &repo.name);
        match issues
            .create_label(&label, "6B6B92", "verctl Version PR")
            .await
        {
            Ok(_) => {}
            Err(octocrab::Error::GitHub { source, .. }) if source.status_code.as_u16() == 422 => {}
            Err(error) => return Err(error).context("create version label"),
        }
        issues
            .add_labels(number, &[label])
            .await
            .context("label Version PR")?;
        Ok(())
    })
}

/// Create `tag` at `sha` if it is missing. Returns the release HTML URL.
///
/// An existing release is not trusted: GitHub's `target_commitish` on the
/// Release object is a branch name more often than a SHA. The git ref is
/// re-read either way, and a tag that names another commit fails.
pub fn ensure_release(
    token: &str,
    repo: &Repo,
    tag: &str,
    name: &str,
    body: &str,
    sha: &str,
) -> Result<String> {
    block(async {
        let crab = client(token)?;
        ensure_release_with(&crab, repo, tag, name, body, sha).await
    })
}

async fn ensure_release_with(
    crab: &Octocrab,
    repo: &Repo,
    tag: &str,
    name: &str,
    body: &str,
    sha: &str,
) -> Result<String> {
    let repos = crab.repos(&repo.owner, &repo.name);
    let releases = repos.releases();
    match releases.get_by_tag(tag).await {
        Ok(release) => {
            require_tag_at(crab, repo, tag, sha).await?;
            return Ok(release.html_url.to_string());
        }
        Err(octocrab::Error::GitHub { source, .. }) if source.status_code.as_u16() == 404 => {}
        Err(error) => return Err(error).context("get release by tag"),
    }
    let release = releases
        .create(tag)
        .name(name)
        .body(body)
        .target_commitish(sha)
        .send()
        .await
        .context("create GitHub release")?;
    require_tag_at(crab, repo, tag, sha).await?;
    Ok(release.html_url.to_string())
}

async fn require_tag_at(crab: &Octocrab, repo: &Repo, tag: &str, sha: &str) -> Result<()> {
    let named = tag_commit_sha(crab, repo, tag).await?;
    ensure!(
        named.eq_ignore_ascii_case(sha),
        "tag {tag} names {named}, not the published commit {sha}"
    );
    Ok(())
}

async fn tag_commit_sha(crab: &Octocrab, repo: &Repo, tag: &str) -> Result<String> {
    let git_ref = crab
        .repos(&repo.owner, &repo.name)
        .get_ref(&octocrab::params::repos::Reference::Tag(tag.to_owned()))
        .await
        .with_context(|| format!("read tag {tag}"))?;
    match git_ref.object {
        octocrab::models::repos::Object::Commit { sha, .. } => Ok(sha),
        octocrab::models::repos::Object::Tag { sha: tag_sha, .. } => {
            peel_tag(crab, repo, tag, &tag_sha).await
        }
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
        .with_context(|| format!("peel tag {tag}"))?;
    Ok(peeled.object.sha)
}

/// Attach `tarball` to the release for `tag`.
pub fn upload_release_asset(token: &str, repo: &Repo, tag: &str, tarball: &Path) -> Result<String> {
    let name = tarball
        .file_name()
        .and_then(|name| name.to_str())
        .context("asset name")?
        .to_owned();
    let body =
        bytes::Bytes::from(std::fs::read(tarball).with_context(|| tarball.display().to_string())?);
    block(async {
        let crab = client(token)?;
        let repos = crab.repos(&repo.owner, &repo.name);
        let releases = repos.releases();
        let release = releases
            .get_by_tag(tag)
            .await
            .context("get release by tag")?;
        match releases
            .upload_asset(release.id.into_inner(), &name, body)
            .send()
            .await
        {
            Ok(asset) => Ok(asset.browser_download_url.to_string()),
            Err(error)
                if format!("{error:#}")
                    .to_ascii_lowercase()
                    .contains("already") =>
            {
                Ok(format!("already {name}"))
            }
            Err(error) => Err(error).context("upload release asset"),
        }
    })
}

fn client(token: &str) -> Result<Octocrab> {
    Octocrab::builder()
        .personal_token(token.to_owned())
        .build()
        .context("GitHub client")
}

fn block<F, T>(fut: F) -> Result<T>
where
    F: std::future::Future<Output = Result<T>>,
{
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("tokio")?
        .block_on(fut)
}

#[cfg(test)]
mod tests {
    use super::{Octocrab, parse_remote, parse_slug};
    use wiremock::matchers::{body_partial_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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
    fn labels_from_event_reads_pull_request() {
        let raw = r#"{
            "pull_request": {
                "labels": [
                    {"name": "verctl:version"},
                    {"name": "do-not-merge"}
                ]
            }
        }"#;
        assert_eq!(
            super::labels_from_event(raw),
            vec!["verctl:version", "do-not-merge"]
        );
        assert_eq!(super::labels_from_event("{}"), Vec::<String>::new());
        assert_eq!(super::labels_from_event("not json"), Vec::<String>::new());
        assert_eq!(
            super::labels_from_event(r#"{"issue":{"labels":[{"name":"verctl:version"}]}}"#),
            Vec::<String>::new()
        );
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

    fn existing_release() -> Mock {
        Mock::given(method("GET"))
            .and(path(format!("/repos/{OWNER}/{NAME}/releases/tags/{TAG}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(release_json("main")))
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

    #[tokio::test]
    async fn create_tags_the_published_commit_not_the_default_branch() {
        let server = MockServer::start().await;
        missing_release().mount(&server).await;
        create_at_published_sha().mount(&server).await;
        lightweight_tag(PUBLISHED).mount(&server).await;
        let url =
            super::ensure_release_with(&crab(&server.uri()), &repo(), TAG, TAG, "notes", PUBLISHED)
                .await
                .unwrap();
        assert!(url.contains(TAG), "{url}");
    }

    #[tokio::test]
    async fn create_fails_when_the_tag_lands_elsewhere() {
        let server = MockServer::start().await;
        missing_release().mount(&server).await;
        create_at_published_sha().mount(&server).await;
        lightweight_tag(OTHER).mount(&server).await;
        let error =
            super::ensure_release_with(&crab(&server.uri()), &repo(), TAG, TAG, "notes", PUBLISHED)
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
        let url =
            super::ensure_release_with(&crab(&server.uri()), &repo(), TAG, TAG, "notes", PUBLISHED)
                .await
                .unwrap();
        assert!(url.contains(TAG), "{url}");
    }

    #[tokio::test]
    async fn existing_release_at_another_commit_fails() {
        let server = MockServer::start().await;
        existing_release().mount(&server).await;
        lightweight_tag(OTHER).mount(&server).await;
        let error =
            super::ensure_release_with(&crab(&server.uri()), &repo(), TAG, TAG, "notes", PUBLISHED)
                .await
                .unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains(OTHER), "{message}");
        assert!(message.contains(PUBLISHED), "{message}");
    }

    #[tokio::test]
    async fn annotated_tag_peels_to_the_commit() {
        let server = MockServer::start().await;
        existing_release().mount(&server).await;
        annotated_tag().mount(&server).await;
        peeled_tag_object(PUBLISHED).mount(&server).await;
        let url =
            super::ensure_release_with(&crab(&server.uri()), &repo(), TAG, TAG, "notes", PUBLISHED)
                .await
                .unwrap();
        assert!(url.contains(TAG), "{url}");
    }
}
