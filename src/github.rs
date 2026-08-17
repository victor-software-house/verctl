use anyhow::{Context, Result, ensure};
use octocrab::Octocrab;
use octocrab::params::State;
use std::env;
use std::path::Path;

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
        .or_else(|| value.get("issue"))
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

/// Create `tag` if it is missing. Returns the release HTML URL.
pub fn ensure_release(
    token: &str,
    repo: &Repo,
    tag: &str,
    name: &str,
    body: &str,
) -> Result<String> {
    block(async {
        let crab = client(token)?;
        let repos = crab.repos(&repo.owner, &repo.name);
        let releases = repos.releases();
        match releases.get_by_tag(tag).await {
            Ok(release) => return Ok(release.html_url.to_string()),
            Err(octocrab::Error::GitHub { source, .. }) if source.status_code.as_u16() == 404 => {}
            Err(error) => return Err(error).context("get release by tag"),
        }
        let release = releases
            .create(tag)
            .name(name)
            .body(body)
            .send()
            .await
            .context("create GitHub release")?;
        Ok(release.html_url.to_string())
    })
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
    use super::{parse_remote, parse_slug};

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
    }

    #[test]
    fn rejects_non_github() {
        assert!(parse_remote("https://gitlab.com/acme/app.git").is_err());
    }
}
