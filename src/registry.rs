//! Whether a registry already lists a package version.
//!
//! `publish` asks before it runs a publisher, so a rerun after a partial
//! failure skips the upload, and asks again afterwards, so a publisher that
//! exits zero without publishing fails the run. A stock cargo package bound for
//! crates.io is read from the crates.io sparse index; any other package asks
//! through its recipe's `query`, or has no check and keeps the stderr match.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};
use semver::Version;
use toml_edit::{DocumentMut, Item};

use crate::process;

/// The crates.io sparse index root.
///
/// The index is public, needs no auth, and is Cargo's own source of truth for
/// crates.io: <https://doc.rust-lang.org/cargo/reference/registry-index.html#sparse-protocol>.
pub const CRATES_IO_INDEX: &str = "https://index.crates.io";

/// The registry name Cargo gives crates.io, in `publish`, `--registry`, and
/// `registry.default`.
const CRATES_IO: &str = "crates-io";

/// How long one `query` command may run.
const QUERY_TIMEOUT: Duration = Duration::from_mins(1);

/// How long one sparse index request may take.
const INDEX_TIMEOUT: Duration = Duration::from_secs(30);

/// The pauses between the checks after a publish. The crates.io index and
/// npm-compatible metadata can trail a successful upload by tens of seconds,
/// so one check right after the publisher exits would fail a good release.
/// The total stays near two minutes, short enough that a publisher which
/// exited zero without publishing still fails the job promptly.
pub const LISTING_DELAYS: [Duration; 6] = [
    Duration::from_secs(2),
    Duration::from_secs(4),
    Duration::from_secs(8),
    Duration::from_secs(16),
    Duration::from_secs(30),
    Duration::from_secs(60),
];

/// How `publish` decides whether a package version is already published.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryCheck {
    /// Read the crate's file in the crates.io sparse index.
    CratesIo {
        /// The crate name as the manifest spells it.
        name: String,
    },
    /// Run this argv in `dir`; it exits zero and prints the version when the
    /// registry has it.
    Query {
        /// The expanded command.
        argv: Vec<String>,
        /// The package directory, where the publisher's registry config
        /// (`bunfig.toml`, `.npmrc`) is in effect.
        dir: PathBuf,
    },
    /// No registry check: the publish succeeds on a zero exit, or on stderr
    /// saying the version already exists.
    Unchecked,
}

impl RegistryCheck {
    /// Whether the registry lists `version` right now.
    ///
    /// # Errors
    ///
    /// Returns an error when the crates.io index answers with a status other
    /// than 200 or 404, or with a line that is not a JSON version record, or
    /// when a `query` cannot be started or times out. A `query` that exits
    /// non-zero is an answer, not an error: the version is not listed.
    pub fn lists(&self, version: &str) -> Result<bool> {
        match self {
            Self::CratesIo { name } => crates_io_lists(CRATES_IO_INDEX, name, version),
            Self::Query { argv, dir } => query_lists(argv, dir, version),
            Self::Unchecked => Ok(false),
        }
    }
}

/// Where a cargo package publishes, as far as verctl can tell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CargoTarget {
    /// Every place Cargo reads the target agrees on crates.io.
    CratesIo,
    /// Some place names another registry, or forbids publishing.
    Other,
    /// A file Cargo would read cannot be parsed, uses `include`, or defers to
    /// the workspace, so the target cannot be known without Cargo.
    Unknown,
}

/// The environment Cargo resolves `registry.default` from, passed in so a test
/// can plant it: `std::env::set_var` is `unsafe`, which this crate forbids.
#[derive(Debug, Clone, Default)]
pub struct CargoEnv {
    /// `CARGO_REGISTRY_DEFAULT`.
    pub registry_default: Option<String>,
    /// `CARGO_HOME`, or `~/.cargo` when unset.
    pub cargo_home: Option<PathBuf>,
}

impl CargoEnv {
    /// Read `CARGO_REGISTRY_DEFAULT`, `CARGO_HOME`, and `HOME` from the
    /// process. An empty value counts as unset.
    #[must_use]
    pub fn from_env() -> Self {
        let non_empty = |key: &str| std::env::var_os(key).filter(|value| !value.is_empty());
        Self {
            registry_default: non_empty("CARGO_REGISTRY_DEFAULT")
                .map(|value| value.to_string_lossy().into_owned()),
            cargo_home: non_empty("CARGO_HOME")
                .map(PathBuf::from)
                .or_else(|| non_empty("HOME").map(|home| PathBuf::from(home).join(".cargo"))),
        }
    }
}

/// Where the stock cargo publisher would publish the package in `manifest`.
///
/// It is crates.io only when every place Cargo reads the target agrees: the
/// package's `registry` in `.ctl/ver.yaml` is unset, empty, or `crates-io`;
/// the manifest's `publish` is absent, `true`, or names only `crates-io`; and
/// the `registry.default` Cargo resolves for `cwd` is unset or `crates-io`.
/// The stock publisher runs from the repository root with `--manifest-path`,
/// so `cwd` is that root, not the package directory.
///
/// `registry.default` resolves in Cargo's documented order, the first that
/// sets it winning: `CARGO_REGISTRY_DEFAULT`, then `.cargo/config.toml` in
/// `cwd` and each parent up to the filesystem root, then the one in
/// `CARGO_HOME`. A directory with both `config` and `config.toml` is read from
/// the extensionless file, as Cargo does:
/// <https://doc.rust-lang.org/cargo/reference/config.html#hierarchical-structure>.
///
/// # Errors
///
/// Returns an error when the manifest cannot be read.
pub fn cargo_target(
    package_registry: Option<&str>,
    manifest: &Path,
    cwd: &Path,
    env: &CargoEnv,
) -> Result<CargoTarget> {
    if package_registry.is_some_and(|registry| !registry.is_empty() && registry != CRATES_IO) {
        return Ok(CargoTarget::Other);
    }
    let raw = std::fs::read_to_string(manifest).with_context(|| manifest.display().to_string())?;
    let Ok(document) = raw.parse::<DocumentMut>() else {
        return Ok(CargoTarget::Unknown);
    };
    Ok(with_default(manifest_publish(&document), cwd, env))
}

/// Where the stock cargo publisher would publish a workspace member, from the
/// `publish` list `cargo metadata` resolves for it: `None` when the member
/// names no registry, which allows any.
///
/// It agrees with [`cargo_target`]: crates.io only when the package's
/// `registry` is unset, empty, or `crates-io`, the list is absent or names
/// only `crates-io`, and the `registry.default` Cargo resolves for `cwd` is
/// unset or `crates-io`.
#[must_use]
pub fn cargo_member_target(
    package_registry: Option<&str>,
    publish: Option<&[String]>,
    cwd: &Path,
    env: &CargoEnv,
) -> CargoTarget {
    if package_registry.is_some_and(|registry| !registry.is_empty() && registry != CRATES_IO) {
        return CargoTarget::Other;
    }
    let listed = match publish {
        None => CargoTarget::CratesIo,
        Some([only]) if only == CRATES_IO => CargoTarget::CratesIo,
        Some(_) => CargoTarget::Other,
    };
    with_default(listed, cwd, env)
}

/// `manifest`, the target the manifest's own `publish` allows, narrowed by the
/// `registry.default` Cargo resolves for `cwd`.
fn with_default(manifest: CargoTarget, cwd: &Path, env: &CargoEnv) -> CargoTarget {
    match manifest {
        CargoTarget::CratesIo => {}
        other @ (CargoTarget::Other | CargoTarget::Unknown) => return other,
    }
    match registry_default(cwd, env) {
        DefaultRegistry::Unset => CargoTarget::CratesIo,
        DefaultRegistry::Named(name) if name == CRATES_IO => CargoTarget::CratesIo,
        DefaultRegistry::Named(_) => CargoTarget::Other,
        DefaultRegistry::Unknown => CargoTarget::Unknown,
    }
}

/// What the manifest's `package.publish` says.
fn manifest_publish(document: &DocumentMut) -> CargoTarget {
    let Some(publish) = document
        .get("package")
        .and_then(|package| package.get("publish"))
    else {
        return CargoTarget::CratesIo;
    };
    if let Some(allowed) = publish.as_bool() {
        return if allowed {
            CargoTarget::CratesIo
        } else {
            CargoTarget::Other
        };
    }
    if let Some(registries) = publish.as_array() {
        let names: Vec<Option<&str>> = registries.iter().map(|name| name.as_str()).collect();
        return if names == [Some(CRATES_IO)] {
            CargoTarget::CratesIo
        } else {
            CargoTarget::Other
        };
    }
    CargoTarget::Unknown
}

/// The `registry.default` Cargo resolves.
#[derive(Debug, PartialEq, Eq)]
enum DefaultRegistry {
    Unset,
    Named(String),
    Unknown,
}

fn registry_default(cwd: &Path, env: &CargoEnv) -> DefaultRegistry {
    if let Some(name) = &env.registry_default {
        return DefaultRegistry::Named(name.clone());
    }
    let dirs = cwd.ancestors().map(|dir| dir.join(".cargo"));
    for dir in dirs.chain(env.cargo_home.iter().cloned()) {
        match config_default(&dir) {
            DefaultRegistry::Unset => {}
            found @ (DefaultRegistry::Named(_) | DefaultRegistry::Unknown) => return found,
        }
    }
    DefaultRegistry::Unset
}

/// `registry.default` from the config file in one Cargo config directory.
fn config_default(dir: &Path) -> DefaultRegistry {
    let Some(path) = ["config", "config.toml"]
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.is_file())
    else {
        return DefaultRegistry::Unset;
    };
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return DefaultRegistry::Unknown;
    };
    let Ok(document) = raw.parse::<DocumentMut>() else {
        return DefaultRegistry::Unknown;
    };
    if document.contains_key("include") {
        return DefaultRegistry::Unknown;
    }
    match document
        .get("registry")
        .and_then(|registry| registry.get("default"))
    {
        None => DefaultRegistry::Unset,
        Some(Item::Value(value)) => value.as_str().map_or(DefaultRegistry::Unknown, |name| {
            DefaultRegistry::Named(name.to_owned())
        }),
        Some(_) => DefaultRegistry::Unknown,
    }
}

/// The index file for `name`, under the layout Cargo documents: one, two, and
/// three character names live in `1/`, `2/`, and `3/<first char>/`, and
/// longer ones in `<first two>/<next two>/`, all lowercased.
fn index_path(name: &str) -> String {
    let name = name.to_ascii_lowercase();
    match name.len() {
        1 => format!("1/{name}"),
        2 => format!("2/{name}"),
        3 => format!("3/{}/{name}", &name[..1]),
        _ => format!("{}/{}/{name}", &name[..2], &name[2..4]),
    }
}

/// Whether the crates.io index under `base` lists `version` of `name`.
///
/// A version matches when it has the same semver precedence, so build
/// metadata never makes a published version look absent.
///
/// # Errors
///
/// Returns an error when `name` is not ASCII, `version` is not semver, the
/// request fails, the index answers with a status other than 200 or 404, or a
/// line in the crate's file is not a JSON version record.
pub fn crates_io_lists(base: &str, name: &str, version: &str) -> Result<bool> {
    ensure!(
        !name.is_empty() && name.is_ascii(),
        "{name:?} is not a crates.io crate name"
    );
    let wanted = Version::parse(version).with_context(|| format!("semver {version:?}"))?;
    let url = format!("{}/{}", base.trim_end_matches('/'), index_path(name));
    let body = crate::github::block(fetch_index(&url))?;
    let Some(body) = body else {
        return Ok(false);
    };
    for line in body.lines().filter(|line| !line.trim().is_empty()) {
        let record: IndexRecord = serde_json::from_str(line)
            .with_context(|| format!("crates.io index line for {name}"))?;
        if Version::parse(&record.vers)
            .is_ok_and(|vers| vers.cmp_precedence(&wanted) == std::cmp::Ordering::Equal)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The release cargo-semver-checks compares `current` of `name` against: the
/// highest version in the crates.io index under `base` that is at most
/// `current`, is not a pre-release, and is not yanked, or else the highest
/// version at most `current`. `None` when the index has no such version.
///
/// This is cargo-semver-checks 0.51.0's own choice (`choose_baseline_version`
/// in `src/rustdoc_gen.rs`), so a decision made from it holds for the
/// baseline the check reads.
///
/// # Errors
///
/// Returns an error under the conditions of [`crates_io_lists`].
pub fn crates_io_baseline(base: &str, name: &str, current: &Version) -> Result<Option<Version>> {
    ensure!(
        !name.is_empty() && name.is_ascii(),
        "{name:?} is not a crates.io crate name"
    );
    let url = format!("{}/{}", base.trim_end_matches('/'), index_path(name));
    let Some(body) = crate::github::block(fetch_index(&url))? else {
        return Ok(None);
    };
    let mut released = Vec::new();
    for line in body.lines().filter(|line| !line.trim().is_empty()) {
        let record: IndexRecord = serde_json::from_str(line)
            .with_context(|| format!("crates.io index line for {name}"))?;
        if let Ok(vers) = Version::parse(&record.vers)
            && vers <= *current
        {
            released.push((vers, record.yanked));
        }
    }
    released.sort();
    let stable = released
        .iter()
        .rev()
        .find(|(vers, yanked)| vers.pre.is_empty() && !yanked);
    Ok(stable.or(released.last()).map(|(vers, _)| vers.clone()))
}

/// One line of a crate's index file.
#[derive(serde::Deserialize)]
struct IndexRecord {
    vers: String,
    #[serde(default)]
    yanked: bool,
}

/// The crate's index file, or `None` when the index has no such crate.
async fn fetch_index(url: &str) -> Result<Option<String>> {
    let client = reqwest::Client::builder()
        .user_agent(user_agent())
        .timeout(INDEX_TIMEOUT)
        .build()
        .context("HTTP client")?;
    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?;
    match response.status() {
        reqwest::StatusCode::OK => Ok(Some(
            response
                .text()
                .await
                .with_context(|| format!("GET {url}"))?,
        )),
        reqwest::StatusCode::NOT_FOUND => Ok(None),
        status => bail!("GET {url}: {status}"),
    }
}

/// crates.io asks automated clients to identify themselves:
/// <https://crates.io/data-access>.
fn user_agent() -> String {
    format!(
        "verctl/{} (+{})",
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_REPOSITORY")
    )
}

/// Run a `query` in `dir`: listed when it exits zero and one of its stdout
/// lines, trimmed of whitespace and quotes, is `version`.
fn query_lists(argv: &[String], dir: &Path, version: &str) -> Result<bool> {
    match process::run_in(argv, dir, QUERY_TIMEOUT) {
        Ok(stdout) => Ok(stdout
            .lines()
            .any(|line| line.trim().trim_matches(['"', '\'']) == version)),
        Err(error) if is_exit_failure(&error) => Ok(false),
        Err(error) => Err(error),
    }
}

/// Whether a [`process::run_in`] error is the command exiting non-zero, which
/// a `query` uses to say "not listed", rather than a failure to run it.
fn is_exit_failure(error: &anyhow::Error) -> bool {
    error.downcast_ref::<process::ExitFailure>().is_some()
}

/// Ask `check` until it lists `version`, pausing for each of `delays` in turn.
///
/// The publish already happened, so a failed check (an index timeout, a 5xx)
/// counts as "not listed yet" for that round rather than failing the release;
/// only the last round's error is reported.
///
/// # Errors
///
/// Returns an error when the registry still does not list `version` after the
/// last pause, carrying the last round's check error when it had one.
pub fn wait_listed(
    check: &RegistryCheck,
    coord: &str,
    version: &str,
    delays: &[Duration],
) -> Result<()> {
    let mut last_error = None;
    for delay in delays {
        std::thread::sleep(*delay);
        match check.lists(version) {
            Ok(true) => return Ok(()),
            Ok(false) => last_error = None,
            Err(error) => last_error = Some(error),
        }
    }
    if let Some(error) = last_error {
        return Err(error.context(format!(
            "{coord}: the registry check failed after the publish"
        )));
    }
    bail!("{coord}: the publisher exited zero, but the registry does not list {version}")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Duration;

    use indoc::indoc;
    use tempfile::TempDir;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::{
        CargoEnv, CargoTarget, RegistryCheck, cargo_target, crates_io_baseline, crates_io_lists,
        index_path, wait_listed,
    };

    #[test]
    fn index_paths_follow_the_cargo_layout() {
        assert_eq!(index_path("a"), "1/a");
        assert_eq!(index_path("ab"), "2/ab");
        assert_eq!(index_path("abc"), "3/a/abc");
        assert_eq!(index_path("serde"), "se/rd/serde");
        assert_eq!(index_path("Inflector"), "in/fl/inflector");
    }

    async fn lists(body: Option<&str>, name: &str, version: &str) -> bool {
        let server = MockServer::start().await;
        let file = format!("/{}", index_path(name));
        let response = body.map_or_else(
            || ResponseTemplate::new(404),
            |body| ResponseTemplate::new(200).set_body_string(body),
        );
        Mock::given(method("GET"))
            .and(path(file))
            .respond_with(response)
            .mount(&server)
            .await;
        let base = server.uri();
        let name = name.to_owned();
        let version = version.to_owned();
        tokio::task::spawn_blocking(move || crates_io_lists(&base, &name, &version))
            .await
            .unwrap()
            .unwrap()
    }

    const DEMO: &str = indoc! {r#"
        {"name":"demo","vers":"0.4.5","deps":[],"cksum":"00","features":{},"yanked":false}
        {"name":"demo","vers":"0.4.6+build.1","deps":[],"cksum":"00","features":{},"yanked":false}
    "#};

    #[tokio::test]
    async fn a_listed_version_is_published() {
        assert!(lists(Some(DEMO), "demo", "0.4.5").await);
    }

    #[tokio::test]
    async fn build_metadata_does_not_hide_a_version() {
        assert!(lists(Some(DEMO), "demo", "0.4.6").await);
    }

    #[tokio::test]
    async fn a_workspace_member_missing_from_the_index_is_not_published() {
        assert!(!lists(Some(DEMO), "demo", "0.4.7").await);
        assert!(!lists(None, "demo", "0.4.5").await);
    }

    #[tokio::test]
    async fn a_crate_name_with_capitals_reads_the_lowercased_file() {
        assert!(lists(Some(DEMO), "Demo", "0.4.5").await);
    }

    async fn baseline(body: Option<&str>, current: &str) -> Option<String> {
        let server = MockServer::start().await;
        let response = body.map_or_else(
            || ResponseTemplate::new(404),
            |body| ResponseTemplate::new(200).set_body_string(body),
        );
        Mock::given(method("GET"))
            .and(path(format!("/{}", index_path("demo"))))
            .respond_with(response)
            .mount(&server)
            .await;
        let base = server.uri();
        let current = semver::Version::parse(current).unwrap();
        tokio::task::spawn_blocking(move || crates_io_baseline(&base, "demo", &current))
            .await
            .unwrap()
            .unwrap()
            .map(|version| version.to_string())
    }

    const RELEASES: &str = indoc! {r#"
        {"name":"demo","vers":"0.4.4","deps":[],"cksum":"00","features":{},"yanked":false}
        {"name":"demo","vers":"0.4.5","deps":[],"cksum":"00","features":{},"yanked":true}
        {"name":"demo","vers":"0.4.6-rc.1","deps":[],"cksum":"00","features":{},"yanked":false}
        {"name":"demo","vers":"0.5.0","deps":[],"cksum":"00","features":{},"yanked":false}
    "#};

    #[tokio::test]
    async fn the_baseline_is_the_newest_stable_release_up_to_the_current_version() {
        assert_eq!(
            baseline(Some(RELEASES), "0.4.6").await.as_deref(),
            Some("0.4.4")
        );
        assert_eq!(
            baseline(Some(RELEASES), "0.5.0").await.as_deref(),
            Some("0.5.0")
        );
    }

    #[tokio::test]
    async fn an_unpublished_crate_has_no_baseline() {
        assert_eq!(baseline(None, "0.0.1").await, None);
        assert_eq!(baseline(Some(RELEASES), "0.4.3").await, None);
    }

    fn crate_dir(manifest: &str) -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("Cargo.toml"), manifest).unwrap();
        dir
    }

    const PLAIN: &str = indoc! {r#"
        [package]
        name = "demo"
        version = "0.4.6"
    "#};

    fn target(dir: &TempDir, registry: Option<&str>, env: &CargoEnv) -> CargoTarget {
        cargo_target(registry, &dir.path().join("Cargo.toml"), dir.path(), env).unwrap()
    }

    fn no_home() -> CargoEnv {
        CargoEnv::default()
    }

    #[test]
    fn a_plain_crate_publishes_to_crates_io() {
        let dir = crate_dir(PLAIN);
        assert_eq!(target(&dir, None, &no_home()), CargoTarget::CratesIo);
        assert_eq!(
            target(&dir, Some("crates-io"), &no_home()),
            CargoTarget::CratesIo
        );
        assert_eq!(target(&dir, Some(""), &no_home()), CargoTarget::CratesIo);
    }

    #[test]
    fn a_package_registry_names_another_target() {
        let dir = crate_dir(PLAIN);
        assert_eq!(
            target(&dir, Some("my-registry"), &no_home()),
            CargoTarget::Other
        );
    }

    #[test]
    fn a_manifest_that_publishes_elsewhere() {
        let dir = crate_dir(indoc! {r#"
            [package]
            name = "demo"
            version = "0.4.6"
            publish = ["my-registry"]
        "#});
        assert_eq!(target(&dir, None, &no_home()), CargoTarget::Other);
        let closed = crate_dir(indoc! {r#"
            [package]
            name = "demo"
            version = "0.4.6"
            publish = false
        "#});
        assert_eq!(target(&closed, None, &no_home()), CargoTarget::Other);
        let inherited = crate_dir(indoc! {r#"
            [package]
            name = "demo"
            version = "0.4.6"
            publish.workspace = true
        "#});
        assert_eq!(target(&inherited, None, &no_home()), CargoTarget::Unknown);
    }

    fn write_config(dir: &std::path::Path, name: &str, body: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(name), body).unwrap();
    }

    const OTHER_DEFAULT: &str = indoc! {r#"
        [registry]
        default = "my-registry"
    "#};

    #[test]
    fn a_parent_directory_config_names_another_default() {
        let parent = TempDir::new().unwrap();
        write_config(&parent.path().join(".cargo"), "config.toml", OTHER_DEFAULT);
        let repo = parent.path().join("repo");
        fs::create_dir_all(&repo).unwrap();
        fs::write(repo.join("Cargo.toml"), PLAIN).unwrap();
        let found = cargo_target(None, &repo.join("Cargo.toml"), &repo, &no_home()).unwrap();
        assert_eq!(found, CargoTarget::Other);
    }

    #[test]
    fn a_runner_whose_cargo_home_names_another_default() {
        let dir = crate_dir(PLAIN);
        let home = TempDir::new().unwrap();
        write_config(home.path(), "config.toml", OTHER_DEFAULT);
        let env = CargoEnv {
            registry_default: None,
            cargo_home: Some(home.path().to_path_buf()),
        };
        assert_eq!(target(&dir, None, &env), CargoTarget::Other);
    }

    #[test]
    fn the_nearest_config_wins_over_cargo_home() {
        let dir = crate_dir(PLAIN);
        write_config(
            &dir.path().join(".cargo"),
            "config.toml",
            indoc! {r#"
                [registry]
                default = "crates-io"
            "#},
        );
        let home = TempDir::new().unwrap();
        write_config(home.path(), "config.toml", OTHER_DEFAULT);
        let env = CargoEnv {
            registry_default: None,
            cargo_home: Some(home.path().to_path_buf()),
        };
        assert_eq!(target(&dir, None, &env), CargoTarget::CratesIo);
    }

    #[test]
    fn the_environment_wins_over_every_file() {
        let dir = crate_dir(PLAIN);
        let env = CargoEnv {
            registry_default: Some("my-registry".into()),
            cargo_home: None,
        };
        assert_eq!(target(&dir, None, &env), CargoTarget::Other);
    }

    #[test]
    fn the_extensionless_config_wins_when_both_exist() {
        let dir = crate_dir(PLAIN);
        let cargo = dir.path().join(".cargo");
        write_config(&cargo, "config", OTHER_DEFAULT);
        write_config(
            &cargo,
            "config.toml",
            indoc! {r#"
                [registry]
                default = "crates-io"
            "#},
        );
        assert_eq!(target(&dir, None, &no_home()), CargoTarget::Other);
    }

    #[test]
    fn an_include_or_a_broken_config_is_unknown() {
        let dir = crate_dir(PLAIN);
        write_config(
            &dir.path().join(".cargo"),
            "config.toml",
            indoc! {r#"
                include = ["shared.toml"]
            "#},
        );
        assert_eq!(target(&dir, None, &no_home()), CargoTarget::Unknown);
        let broken = crate_dir(PLAIN);
        write_config(&broken.path().join(".cargo"), "config.toml", "[registry\n");
        assert_eq!(target(&broken, None, &no_home()), CargoTarget::Unknown);
    }

    #[test]
    fn a_check_that_keeps_failing_is_reported_after_the_last_round() {
        let dir = TempDir::new().unwrap();
        let check = RegistryCheck::Query {
            argv: vec!["verctl-test-no-such-query".to_owned()],
            dir: dir.path().to_path_buf(),
        };
        let error =
            wait_listed(&check, "demo", "1.0.0", &[Duration::ZERO, Duration::ZERO]).unwrap_err();
        assert!(
            format!("{error:#}").starts_with("demo: the registry check failed after the publish"),
            "{error:#}"
        );
    }

    #[test]
    fn a_version_still_unlisted_after_the_last_round_fails() {
        let dir = TempDir::new().unwrap();
        let check = RegistryCheck::Query {
            argv: vec!["false".to_owned()],
            dir: dir.path().to_path_buf(),
        };
        let error = wait_listed(&check, "demo", "1.0.0", &[Duration::ZERO]).unwrap_err();
        assert_eq!(
            error.to_string(),
            "demo: the publisher exited zero, but the registry does not list 1.0.0"
        );
    }
}
