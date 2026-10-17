//! Whether docs.rs built each crates.io crate a version commit released.
//!
//! docs.rs builds every crate published to crates.io, and a failed build stays
//! failed for that version. `verctl docs` waits for those builds after a
//! release, so a crate whose documentation never rendered fails a job instead
//! of going unnoticed until the next release.

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use toml_edit::DocumentMut;

use crate::config::Config;
use crate::registry::RegistryCheck;
use crate::{git, github, publish, publisher, release};

/// The docs.rs root.
pub const DOCS_RS: &str = "https://docs.rs";

/// How long one docs.rs request may take.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// The pause between polls. docs.rs builds take minutes and its queue can
/// take hours, so polling faster only adds load.
pub const POLL_EVERY: Duration = Duration::from_secs(30);

/// What docs.rs reports for one crate version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DocsStatus {
    /// docs.rs built the documentation.
    Built,
    /// docs.rs tried and failed; that version stays without documentation.
    Failed,
    /// docs.rs had not built the version when the wait ran out.
    Unbuilt,
}

/// One crate's docs.rs result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct DocsLine {
    /// The crate name crates.io knows.
    pub name: String,
    /// The released version.
    pub version: String,
    /// What docs.rs reported.
    pub status: DocsStatus,
}

/// A crate version to ask docs.rs about.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Wanted {
    name: String,
    version: String,
}

/// Wait for docs.rs to build each crates.io library crate released at HEAD,
/// for at most `timeout`.
///
/// A crate is checked only when its publish target is crates.io, it has a
/// library target, and the tag `tags.template` names for its version names
/// HEAD on GitHub. `publish` creates the tag only after the registry lists
/// the version, so the tag records that the publish happened; the lookup goes
/// to GitHub, never to local refs, because a CI checkout fetches no tags.
///
/// # Errors
///
/// Returns an error when no token is set, the plan, a manifest, or a tag
/// cannot be read, a docs.rs request fails, or any crate's build failed or
/// was still missing when `timeout` ran out.
pub fn run(config: &Config, root: &Path, timeout: Duration) -> Result<Vec<DocsLine>> {
    let token = release::resolve_token()?;
    let wanted = released_crates(config, root, &token)?;
    let lines = wait_built(DOCS_RS, &wanted, timeout, POLL_EVERY)?;
    let bad: Vec<String> = lines
        .iter()
        .filter(|line| line.status != DocsStatus::Built)
        .map(|line| {
            let status = match line.status {
                DocsStatus::Built => "built",
                DocsStatus::Failed => "failed",
                DocsStatus::Unbuilt => "unbuilt",
            };
            format!("{}@{} {status}", line.name, line.version)
        })
        .collect();
    if !bad.is_empty() {
        bail!("docs.rs did not build {}", bad.join(", "));
    }
    Ok(lines)
}

/// The crates.io library crates whose release tag names HEAD.
fn released_crates(config: &Config, root: &Path, token: &str) -> Result<Vec<Wanted>> {
    let head = git::head_sha(root)?;
    let planned = publish::plan(config, root)?;
    let repo = github::repo(root)?;
    let mut wanted = Vec::new();
    for (spec, entry) in config.packages.iter().zip(&planned.packages) {
        if entry.argv.is_empty() {
            continue;
        }
        let RegistryCheck::CratesIo { name } =
            publisher::registry_check(config, spec, root, &entry.version)?
        else {
            continue;
        };
        if !has_library(&root.join(&spec.path))? {
            continue;
        }
        let Some(tag) = planned
            .tags
            .iter()
            .find(|tag| tag.packages.iter().any(|named| named.name == entry.name))
        else {
            continue;
        };
        let state = github::release_state(token, &repo, &tag.tag)?;
        if state.tag.is_some_and(|sha| sha.eq_ignore_ascii_case(&head)) {
            wanted.push(Wanted {
                name,
                version: entry.version.clone(),
            });
        }
    }
    Ok(wanted)
}

/// Whether the package in `manifest` has a library target, as Cargo resolves
/// it: an explicit `[lib]` table, or `src/lib.rs` found by target
/// auto-discovery unless `package.autolib` is `false`. docs.rs documents no
/// binary-only crate.
/// <https://doc.rust-lang.org/cargo/reference/cargo-targets.html#target-auto-discovery>
///
/// # Errors
///
/// Returns an error when the manifest cannot be read or parsed.
pub fn has_library(manifest: &Path) -> Result<bool> {
    let raw = std::fs::read_to_string(manifest).with_context(|| manifest.display().to_string())?;
    let document = raw
        .parse::<DocumentMut>()
        .with_context(|| manifest.display().to_string())?;
    if document.contains_key("lib") {
        return Ok(true);
    }
    let autolib = document
        .get("package")
        .and_then(|package| package.get("autolib"))
        .and_then(toml_edit::Item::as_bool)
        .unwrap_or(true);
    let dir = manifest.parent().unwrap_or_else(|| Path::new("."));
    Ok(autolib && dir.join("src").join("lib.rs").is_file())
}

/// Poll docs.rs under `base` for each crate until every one has a status or
/// `timeout` runs out, pausing `poll` between rounds.
fn wait_built(
    base: &str,
    wanted: &[Wanted],
    timeout: Duration,
    poll: Duration,
) -> Result<Vec<DocsLine>> {
    let deadline = Instant::now() + timeout;
    let mut remaining: Vec<&Wanted> = wanted.iter().collect();
    let mut lines = Vec::new();
    loop {
        let mut unbuilt = Vec::new();
        for crate_version in remaining {
            match status(base, &crate_version.name, &crate_version.version)? {
                Some(built) => lines.push(DocsLine {
                    name: crate_version.name.clone(),
                    version: crate_version.version.clone(),
                    status: if built {
                        DocsStatus::Built
                    } else {
                        DocsStatus::Failed
                    },
                }),
                None => unbuilt.push(crate_version),
            }
        }
        remaining = unbuilt;
        let now = Instant::now();
        if remaining.is_empty() || now >= deadline {
            break;
        }
        std::thread::sleep(poll.min(deadline - now));
    }
    lines.extend(remaining.into_iter().map(|crate_version| DocsLine {
        name: crate_version.name.clone(),
        version: crate_version.version.clone(),
        status: DocsStatus::Unbuilt,
    }));
    Ok(lines)
}

/// docs.rs's build result for one crate version: `None` while it has not
/// built the version (docs.rs answers 404 until then), otherwise
/// `doc_status`.
fn status(base: &str, name: &str, version: &str) -> Result<Option<bool>> {
    let url = format!(
        "{}/crate/{name}/{version}/status.json",
        base.trim_end_matches('/')
    );
    github::block(fetch_status(&url))
}

#[derive(serde::Deserialize)]
struct StatusJson {
    doc_status: bool,
}

async fn fetch_status(url: &str) -> Result<Option<bool>> {
    let client = reqwest::Client::builder()
        .user_agent(format!(
            "verctl/{} (+{})",
            env!("CARGO_PKG_VERSION"),
            env!("CARGO_PKG_REPOSITORY")
        ))
        .timeout(REQUEST_TIMEOUT)
        .build()
        .context("HTTP client")?;
    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?;
    match response.status() {
        reqwest::StatusCode::OK => {
            let body: StatusJson = response
                .json()
                .await
                .with_context(|| format!("GET {url}"))?;
            Ok(Some(body.doc_status))
        }
        reqwest::StatusCode::NOT_FOUND => Ok(None),
        status => bail!("GET {url}: {status}"),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Duration;

    use indoc::indoc;
    use tempfile::TempDir;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::{DocsStatus, Wanted, has_library, wait_built};

    fn demo() -> Vec<Wanted> {
        vec![Wanted {
            name: "demo".into(),
            version: "0.4.6".into(),
        }]
    }

    const STATUS: &str = "/crate/demo/0.4.6/status.json";

    async fn wait(server: &MockServer, timeout: Duration) -> DocsStatus {
        let base = server.uri();
        tokio::task::spawn_blocking(move || {
            wait_built(&base, &demo(), timeout, Duration::ZERO).unwrap()
        })
        .await
        .unwrap()[0]
            .status
    }

    fn answer(doc_status: bool) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "doc_status": doc_status,
            "version": "0.4.6"
        }))
    }

    #[tokio::test]
    async fn a_build_that_failed() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(STATUS))
            .respond_with(answer(false))
            .mount(&server)
            .await;
        assert_eq!(
            wait(&server, Duration::from_secs(5)).await,
            DocsStatus::Failed
        );
    }

    #[tokio::test]
    async fn a_queued_build() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(STATUS))
            .respond_with(ResponseTemplate::new(404))
            .up_to_n_times(2)
            .with_priority(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(STATUS))
            .respond_with(answer(true))
            .mount(&server)
            .await;
        assert_eq!(
            wait(&server, Duration::from_secs(5)).await,
            DocsStatus::Built
        );
    }

    #[tokio::test]
    async fn a_build_that_never_arrives() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(STATUS))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        assert_eq!(wait(&server, Duration::ZERO).await, DocsStatus::Unbuilt);
    }

    fn manifest(dir: &TempDir, body: &str) -> std::path::PathBuf {
        let path = dir.path().join("Cargo.toml");
        fs::write(&path, body).unwrap();
        path
    }

    const PLAIN: &str = indoc! {r#"
        [package]
        name = "demo"
        version = "0.4.6"
    "#};

    #[test]
    fn a_library_found_by_auto_discovery() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/lib.rs"), "").unwrap();
        assert!(has_library(&manifest(&dir, PLAIN)).unwrap());
    }

    #[test]
    fn a_binary_only_crate() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();
        assert!(!has_library(&manifest(&dir, PLAIN)).unwrap());
    }

    #[test]
    fn an_explicit_lib_table_or_autolib_off() {
        let dir = TempDir::new().unwrap();
        let explicit = manifest(
            &dir,
            indoc! {r#"
                [package]
                name = "demo"
                version = "0.4.6"

                [lib]
                path = "lib/demo.rs"
            "#},
        );
        assert!(has_library(&explicit).unwrap());
        let off = TempDir::new().unwrap();
        fs::create_dir_all(off.path().join("src")).unwrap();
        fs::write(off.path().join("src/lib.rs"), "").unwrap();
        let path = manifest(
            &off,
            indoc! {r#"
                [package]
                name = "demo"
                version = "0.4.6"
                autolib = false
            "#},
        );
        assert!(!has_library(&path).unwrap());
    }
}
