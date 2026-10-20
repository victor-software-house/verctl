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
    /// docs.rs still listed the version without documentation when the wait
    /// ran out: its build failed, and that version stays without
    /// documentation, or it was still running after the whole wait.
    Failed,
    /// docs.rs had not started a build of the version when the wait ran
    /// out, or could not be asked.
    Unbuilt,
}

/// What the last poll learned about a crate docs.rs has not built.
enum Pending {
    /// docs.rs answered 404: it has not started a build.
    Unlisted,
    /// docs.rs listed the version without documentation: a build that is
    /// running or that failed. A later failed request keeps this state and
    /// carries its error, so a request that fails after docs.rs listed the
    /// version does not turn a failed build into an unbuilt one.
    Undocumented(Option<String>),
    /// The request failed, with this error.
    Unreachable(String),
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
    /// For a crate not built at the deadline, the error the last poll hit,
    /// if it hit one: docs.rs was unreachable or answered with a server
    /// error, so "unbuilt" may mean "could not ask", and "failed" may mean
    /// docs.rs could not be asked again after it listed the version.
    pub last_error: Option<String>,
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
/// library target, the tag `tags.template` names for its version names HEAD
/// on GitHub, and the crates.io index lists the version. `publish` creates a
/// crates.io package's tag before the upload, so the tag says which commit
/// released the version and the index says the upload happened; the tag
/// lookup goes to GitHub, never to local refs, because a CI checkout fetches
/// no tags.
///
/// # Errors
///
/// Returns an error when no token is set, the plan, a manifest, a tag, or the
/// crates.io index cannot be read, or any crate's build failed or was still
/// missing when `timeout` ran out, with the last docs.rs error when polling hit
/// one.
pub fn run(config: &Config, root: &Path, timeout: Duration) -> Result<Vec<DocsLine>> {
    let token = release::resolve_token()?;
    let wanted = released_crates(config, root, &token)?;
    let lines = wait_built(DOCS_RS, &wanted, timeout, POLL_EVERY);
    let bad: Vec<String> = lines
        .iter()
        .filter(|line| line.status != DocsStatus::Built)
        .map(|line| {
            let status = match line.status {
                DocsStatus::Built => "built",
                DocsStatus::Failed => "failed",
                DocsStatus::Unbuilt => "unbuilt",
            };
            match &line.last_error {
                Some(error) => format!("{}@{} {status} ({error})", line.name, line.version),
                None => format!("{}@{} {status}", line.name, line.version),
            }
        })
        .collect();
    if !bad.is_empty() {
        bail!("docs.rs did not build {}", bad.join(", "));
    }
    Ok(lines)
}

/// The crates.io library crates whose release tag names HEAD, each library
/// member of a workspace package included.
fn released_crates(config: &Config, root: &Path, token: &str) -> Result<Vec<Wanted>> {
    let head = git::head_sha(root)?;
    let planned = publish::plan(config, root)?;
    let repo = github::repo(root)?;
    let mut wanted = Vec::new();
    for (spec, entry) in config.packages.iter().zip(&planned.packages) {
        if entry.argv.is_empty() {
            continue;
        }
        let mut libraries = Vec::new();
        for upload in publisher::uploads(config, spec, root, entry)? {
            let RegistryCheck::CratesIo { name } = &upload.check else {
                continue;
            };
            if has_library(&upload.manifest)? {
                libraries.push((
                    upload.check.clone(),
                    Wanted {
                        name: name.clone(),
                        version: upload.version,
                    },
                ));
            }
        }
        if libraries.is_empty() {
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
        if !state.tag.is_some_and(|sha| sha.eq_ignore_ascii_case(&head)) {
            continue;
        }
        // Only a release at HEAD asks the index, one request per library.
        for (check, library) in libraries {
            if check.lists(&library.version)? {
                wanted.push(library);
            }
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
///
/// Only `doc_status: true` ends the wait for a crate. docs.rs answers
/// `doc_status: false` both while a build runs and after it failed, so a
/// crate that still answers it at the deadline is `failed`, and one docs.rs
/// never listed is `unbuilt`.
///
/// A failed request counts as "not built yet" for that round: over a wait of
/// up to an hour, one 502 or one slow answer from docs.rs must not fail a
/// build docs.rs later finishes. The last error stays on the line, so a crate
/// still unbuilt at the deadline says whether docs.rs could be asked at all.
fn wait_built(base: &str, wanted: &[Wanted], timeout: Duration, poll: Duration) -> Vec<DocsLine> {
    let deadline = Instant::now() + timeout;
    let mut remaining: Vec<(&Wanted, Pending)> = wanted
        .iter()
        .map(|crate_version| (crate_version, Pending::Unlisted))
        .collect();
    let mut lines = Vec::new();
    loop {
        let mut unbuilt = Vec::new();
        for (crate_version, previous) in remaining {
            match status(base, &crate_version.name, &crate_version.version) {
                Ok(Some(true)) => lines.push(DocsLine {
                    name: crate_version.name.clone(),
                    version: crate_version.version.clone(),
                    status: DocsStatus::Built,
                    last_error: None,
                }),
                Ok(Some(false)) => unbuilt.push((crate_version, Pending::Undocumented(None))),
                Ok(None) => unbuilt.push((crate_version, Pending::Unlisted)),
                Err(error) => {
                    let error = format!("{error:#}");
                    let next = match previous {
                        Pending::Undocumented(_) => Pending::Undocumented(Some(error)),
                        Pending::Unlisted | Pending::Unreachable(_) => Pending::Unreachable(error),
                    };
                    unbuilt.push((crate_version, next));
                }
            }
        }
        remaining = unbuilt;
        let now = Instant::now();
        if remaining.is_empty() || now >= deadline {
            break;
        }
        std::thread::sleep(poll.min(deadline - now));
    }
    lines.extend(remaining.into_iter().map(|(crate_version, pending)| {
        let (status, last_error) = match pending {
            Pending::Undocumented(error) => (DocsStatus::Failed, error),
            Pending::Unlisted => (DocsStatus::Unbuilt, None),
            Pending::Unreachable(error) => (DocsStatus::Unbuilt, Some(error)),
        };
        DocsLine {
            name: crate_version.name.clone(),
            version: crate_version.version.clone(),
            status,
            last_error,
        }
    }));
    lines
}

/// docs.rs's `doc_status` for one crate version, or `None` while docs.rs
/// answers 404.
///
/// docs.rs adds a release when its build starts, and reports `doc_status`
/// false until the build stores documentation, so `Some(false)` is a running
/// build or a failed one; the endpoint says nothing else.
/// <https://github.com/rust-lang/docs.rs/blob/4259a798bb58835b634f4d75c4481f2f17bdb1d5/crates/bin/docs_rs_builder/src/docbuilder/rustwide_builder.rs#L220>
/// <https://github.com/rust-lang/docs.rs/blob/4259a798bb58835b634f4d75c4481f2f17bdb1d5/crates/bin/docs_rs_web/src/match_release.rs#L107-L109>
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
            let raw = response
                .text()
                .await
                .with_context(|| format!("GET {url}"))?;
            let body: StatusJson =
                serde_json::from_str(&raw).with_context(|| format!("GET {url}: {raw}"))?;
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
        tokio::task::spawn_blocking(move || wait_built(&base, &demo(), timeout, Duration::ZERO))
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
        assert_eq!(wait(&server, Duration::ZERO).await, DocsStatus::Failed);
    }

    #[tokio::test]
    async fn a_listed_build_stays_failed_when_docs_rs_stops_answering() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(STATUS))
            .respond_with(answer(false))
            .up_to_n_times(1)
            .with_priority(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(STATUS))
            .respond_with(ResponseTemplate::new(502))
            .mount(&server)
            .await;
        let base = server.uri();
        let line = tokio::task::spawn_blocking(move || {
            wait_built(
                &base,
                &demo(),
                Duration::from_secs(2),
                Duration::from_millis(10),
            )
        })
        .await
        .unwrap()
        .remove(0);
        assert_eq!(line.status, DocsStatus::Failed);
        assert!(line.last_error.is_some_and(|error| error.contains("502")));
    }

    #[tokio::test]
    async fn a_running_build() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(STATUS))
            .respond_with(answer(false))
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
    async fn a_server_error_is_retried_until_the_build_arrives() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(STATUS))
            .respond_with(ResponseTemplate::new(502))
            .up_to_n_times(1)
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
