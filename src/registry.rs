//! Whether a registry already lists a package version.
//!
//! `release` checks the registry before it runs a publisher, so a rerun after
//! a partial failure skips the upload. It checks again afterwards, so a
//! publisher that exits zero without publishing fails the run.
//!
//! A cargo package that the stock publisher sends to crates.io is read from
//! the crates.io sparse index. Any other package is checked through its
//! recipe's `query`, or has no check and keeps the stderr match.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{env, fmt, fs, thread};

use anyhow::{Context, Result, bail, ensure};
use reqwest::StatusCode;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue, RETRY_AFTER};
use semver::Version;
use toml_edit::{DocumentMut, Item};

use crate::github::Repo;
use crate::{github, http_date, lookup, process};

/// The root URL of the crates.io sparse index.
///
/// The index is public and needs no auth. It is the listing Cargo itself
/// trusts for crates.io, as the [sparse protocol] documents.
///
/// [sparse protocol]: https://doc.rust-lang.org/cargo/reference/registry-index.html#sparse-protocol
pub const CRATES_IO_INDEX: &str = "https://index.crates.io";

/// The root URL of the crates.io API, which [`crates_io_user_owners`],
/// [`crates_io_newest_trust`], and [`crates_io_github_configs`] read.
pub const CRATES_IO_API: &str = "https://crates.io/api/v1";

/// The crates.io a run reads, and the owner's token its `trust` proof sends
/// there.
///
/// `release --crates-io-index` and `--crates-io-api` set the two addresses,
/// so a run can read crates.io's staging instance, or a test a recorded one.
///
/// Its `Debug` output shows whether the token is set and hides it, so an
/// error or a log line that prints it holds no secret.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CratesIo<'a> {
    /// The sparse index root, such as [`CRATES_IO_INDEX`], which the
    /// completeness check and the check after each upload read.
    pub index: &'a str,
    /// The API root, such as [`CRATES_IO_API`], which the release proof reads.
    pub api: &'a str,
    /// An owner's token for the `trust` proof, from
    /// [`crate::registry_token::OWNER`], or `None` to skip it.
    pub owner_token: Option<&'a str>,
}

impl CratesIo<'static> {
    /// crates.io itself, with no owner's token.
    pub const PUBLIC: Self = Self {
        index: CRATES_IO_INDEX,
        api: CRATES_IO_API,
        owner_token: None,
    };
}

impl fmt::Debug for CratesIo<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CratesIo")
            .field("index", &self.index)
            .field("api", &self.api)
            .field("owner_token", &self.owner_token.map(|_| "<redacted>"))
            .finish()
    }
}

/// The registry name Cargo gives crates.io, in `publish`, `--registry`, and
/// `registry.default`.
const CRATES_IO: &str = "crates-io";

/// How long one `query` command may run.
const QUERY_TIMEOUT: Duration = Duration::from_mins(1);

/// How long one sparse index request may take.
const INDEX_TIMEOUT: Duration = Duration::from_secs(30);

/// How long to wait between the checks after a publish. The crates.io index
/// and npm-compatible metadata can trail a successful upload by tens of
/// seconds. One check right after the publisher exits would then fail a good
/// release.
///
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

/// How `release` decides whether a package version is already published.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
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
    /// Returns an error when:
    ///
    /// - the crates.io index answers with a status other than 200 or 404;
    /// - the crates.io index answers with a line that is not a JSON version
    ///   record;
    /// - a `query` cannot be started or times out.
    ///
    /// A `query` that exits non-zero gives an answer instead of an error: the
    /// version is not listed.
    ///
    /// Only a registry that gave no answer is a [`lookup::Unavailable`]. That
    /// covers an index request that could not connect or timed out, an index
    /// 5xx, and a `query` that timed out. A line that does not parse and a
    /// `query` that cannot be started stay errors.
    ///
    /// A crate is read from the sparse index at `index`, such as
    /// [`CRATES_IO_INDEX`].
    ///
    /// A `query` runs with `env` set. `release` passes the registry token
    /// under the variable its publisher reads, because a query against a
    /// private registry needs the same credential. A check that publishes
    /// nothing passes none.
    pub fn lists(&self, index: &str, version: &str, env: &[(&str, &str)]) -> Result<bool> {
        match self {
            Self::CratesIo { name } => crates_io_lists(index, name, version),
            Self::Query { argv, dir } => query_lists(argv, dir, version, env),
            Self::Unchecked => Ok(false),
        }
    }
}

/// Where a cargo package publishes, as far as verctl can tell.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CargoTarget {
    /// Every place Cargo reads the target names crates.io.
    CratesIo,
    /// Some place names another registry, or forbids publishing.
    Other,
    /// A file Cargo would read cannot be parsed, uses `include`, or defers to
    /// the workspace, so the target cannot be known without Cargo.
    Unknown,
}

/// The environment Cargo resolves `registry.default` from, passed in so a test
/// can plant it: `std::env::set_var` is `unsafe`, which this crate denies.
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
        let non_empty = |key: &str| env::var_os(key).filter(|value| !value.is_empty());
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
/// It is crates.io only when all of these hold:
///
/// - the package's `registry` in `.ctl/ver.yaml` is unset, empty, or
///   `crates-io`;
/// - the manifest's `publish` is absent, `true`, or names only `crates-io`;
/// - the `registry.default` Cargo resolves for `cwd` is unset or `crates-io`.
///
/// The stock publisher runs from the repository root with `--manifest-path`,
/// so `cwd` is that root instead of the package directory.
///
/// `registry.default` resolves in Cargo's documented order. The first source
/// that sets it wins:
///
/// 1. `CARGO_REGISTRY_DEFAULT`;
/// 2. `.cargo/config.toml` in `cwd` and each parent up to the filesystem root;
/// 3. the config in `CARGO_HOME`.
///
/// A directory with both `config` and `config.toml` is read from
/// the extensionless file, as Cargo does ([hierarchical structure]).
///
/// # Errors
///
/// Returns an error when the manifest cannot be read.
///
/// [hierarchical structure]: https://doc.rust-lang.org/cargo/reference/config.html#hierarchical-structure
pub fn cargo_target(
    package_registry: Option<&str>,
    manifest: &Path,
    cwd: &Path,
    env: &CargoEnv,
) -> Result<CargoTarget> {
    if package_registry.is_some_and(|registry| !registry.is_empty() && registry != CRATES_IO) {
        return Ok(CargoTarget::Other);
    }
    let raw = fs::read_to_string(manifest).with_context(|| manifest.display().to_string())?;
    let Ok(document) = raw.parse::<DocumentMut>() else {
        return Ok(CargoTarget::Unknown);
    };
    Ok(with_default(manifest_publish(&document), cwd, env))
}

/// Where the stock cargo publisher would publish a workspace member, from the
/// `publish` list `cargo metadata` resolves for it: `None` when the member
/// names no registry, which allows any.
///
/// It follows [`cargo_target`]. The result is crates.io only when all of
/// these hold:
///
/// - the package's `registry` is unset, empty, or `crates-io`;
/// - the list is absent or names only `crates-io`;
/// - the `registry.default` Cargo resolves for `cwd` is unset or `crates-io`.
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

/// What the manifest's `package.publish` holds.
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
    let Ok(raw) = fs::read_to_string(&path) else {
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

/// What the crates.io index shows of one version of a crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Listing {
    /// The index has no file for the crate, so no version of it was published.
    NoCrate,
    /// The index has the crate and not this version.
    Unlisted,
    /// The index lists the version.
    Listed,
}

/// What the crates.io index under `base` shows of `version` of `name`, from
/// one read of the crate's file.
///
/// A version matches when it has the same semver precedence, so build
/// metadata never makes a published version look absent.
///
/// # Errors
///
/// Returns an error when:
///
/// - `name` is not ASCII;
/// - `version` is not semver;
/// - the request fails;
/// - the index answers with a status other than 200 or 404;
/// - a line in the crate's file is not a JSON version record.
pub fn crates_io_listing(base: &str, name: &str, version: &str) -> Result<Listing> {
    ensure!(
        !name.is_empty() && name.is_ascii(),
        "{name:?} is not a crates.io crate name"
    );
    let wanted = Version::parse(version).with_context(|| format!("semver {version:?}"))?;
    let url = format!("{}/{}", base.trim_end_matches('/'), index_path(name));
    let body = github::block(fetch(&url, None))?;
    let Some(body) = body else {
        return Ok(Listing::NoCrate);
    };
    for line in body.lines().filter(|line| !line.trim().is_empty()) {
        let record: IndexRecord = serde_json::from_str(line)
            .with_context(|| format!("crates.io index line for {name}"))?;
        if Version::parse(&record.vers)
            .is_ok_and(|vers| vers.cmp_precedence(&wanted) == Ordering::Equal)
        {
            return Ok(Listing::Listed);
        }
    }
    Ok(Listing::Unlisted)
}

/// Whether the crates.io index under `base` lists `version` of `name`, as
/// [`crates_io_listing`] reads it.
///
/// # Errors
///
/// Returns an error under the conditions of [`crates_io_listing`].
pub fn crates_io_lists(base: &str, name: &str, version: &str) -> Result<bool> {
    Ok(crates_io_listing(base, name, version)? == Listing::Listed)
}

/// The release cargo-semver-checks compares `current` of `name` against.
/// It is the highest version that the crates.io index under `base` lists, is
/// at most `current`, and is neither a pre-release nor yanked. Otherwise it is
/// the highest version at most `current`. `None` when the index has no such
/// version.
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
    let Some(body) = github::block(fetch(&url, None))? else {
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

/// The logins of the user owners of `name`, from the crates.io API under
/// `base`, which needs no token. A team owner has a login too, and is left
/// out. `None` when crates.io does not know the crate.
///
/// # Errors
///
/// Returns an error when:
///
/// - `name` is not ASCII;
/// - the request fails;
/// - the API answers with a status other than 200 or 404;
/// - the body is not the owners JSON.
///
/// Only an API that gave no answer is a [`lookup::Unavailable`], under the
/// conditions of [`RegistryCheck::lists`].
pub fn crates_io_user_owners(base: &str, name: &str) -> Result<Option<Vec<String>>> {
    ensure!(
        !name.is_empty() && name.is_ascii(),
        "{name:?} is not a crates.io crate name"
    );
    let url = format!("{}/crates/{name}/owners", base.trim_end_matches('/'));
    let Some(body) = github::block(fetch(&url, None))? else {
        return Ok(None);
    };
    let owners: Owners =
        serde_json::from_str(&body).with_context(|| format!("crates.io owners of {name}"))?;
    Ok(Some(
        owners
            .users
            .into_iter()
            .filter(|owner| owner.kind == "user")
            .map(|owner| owner.login)
            .collect(),
    ))
}

/// How a crate's newest version reached crates.io, as its `trustpub_data`
/// records it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TrustedUpload {
    /// Trusted publishing from a GitHub Actions run.
    GitHub {
        /// The `owner/name` of the repository whose workflow published it.
        repository: String,
    },
    /// Trusted publishing from a GitLab CI pipeline.
    GitLab {
        /// The `group/project` path of the project that published it.
        project_path: String,
    },
    /// Not through trusted publishing: a token upload, or no version at all.
    Untrusted,
}

/// How the newest version of `name` reached crates.io, from the crates.io
/// API under `base`, which needs no token. The newest is the first version
/// `sort=date` lists. A crate crates.io does not know, or one with no
/// version, is [`TrustedUpload::Untrusted`].
///
/// # Errors
///
/// Returns an error under the conditions of [`crates_io_user_owners`], or
/// when `trustpub_data` names a provider other than `github` or `gitlab`.
pub fn crates_io_newest_trust(base: &str, name: &str) -> Result<TrustedUpload> {
    ensure!(
        !name.is_empty() && name.is_ascii(),
        "{name:?} is not a crates.io crate name"
    );
    let url = format!(
        "{}/crates/{name}/versions?per_page=1&sort=date",
        base.trim_end_matches('/')
    );
    let Some(body) = github::block(fetch(&url, None))? else {
        return Ok(TrustedUpload::Untrusted);
    };
    let listed: Versions =
        serde_json::from_str(&body).with_context(|| format!("crates.io versions of {name}"))?;
    Ok(
        match listed
            .versions
            .into_iter()
            .next()
            .and_then(|version| version.trustpub_data)
        {
            Some(TrustpubData::Github { repository }) => TrustedUpload::GitHub { repository },
            Some(TrustpubData::Gitlab { project_path }) => TrustedUpload::GitLab { project_path },
            None => TrustedUpload::Untrusted,
        },
    )
}

/// One GitHub trusted publisher entry of a crate: the repository and the
/// workflow file whose OIDC token crates.io accepts for it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[non_exhaustive]
pub struct GithubConfig {
    /// The repository's owner, as GitHub spells it.
    pub repository_owner: String,
    /// The repository's name.
    pub repository_name: String,
    /// The calling workflow's file name under `.github/workflows/`.
    pub workflow_filename: String,
    /// The deployment environment the run must use, or `None` for any.
    pub environment: Option<String>,
}

/// What crates.io answered to a request for the trusted publishers of a crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustedPublishers {
    /// The entries, in the order crates.io lists them.
    Listed(Vec<GithubConfig>),
    /// crates.io answered 400, which is how it refuses a token whose user does
    /// not own the crate (`list_by_crate` in crates.io's
    /// `src/controllers/trustpub/github_configs/list.rs`). The refusal belongs
    /// to this crate alone, so the caller can go on to the next one.
    Refused {
        /// crates.io's reason: the `detail` of each of its errors joined by
        /// `; `, or its body when that is not an errors document.
        detail: String,
    },
}

/// The most pages of trusted publisher entries one crate is read for.
/// crates.io pages them by the default page size, and a crate has a handful,
/// so more pages than this means its paging is broken and the run stops
/// instead of reading on.
const MAX_TRUSTED_PUBLISHER_PAGES: usize = 100;

/// The GitHub trusted publisher entries of `name`, from the crates.io API
/// under `base`.
///
/// crates.io serves them only to an owner. `token`, an owner's API token,
/// goes in the `Authorization` header of these requests and nowhere else.
/// No URL, log line, or error holds it.
///
/// Each page's `meta.next_page` is a query string for the same path
/// (`query_with_params` in crates.io's `src/util/request_helpers.rs`). It is
/// followed until it is null, a page holds no entries, or the entries reach
/// `meta.total`. A `next_page` that leads to a page already read, and more
/// than 100 pages, are errors, because following
/// them may never end.
///
/// # Errors
///
/// Returns an error when:
///
/// - `name` is not ASCII;
/// - `token` cannot be an HTTP header value;
/// - a request fails;
/// - the API answers with a status other than 200 or 400, such as 403 for a
///   token it does not accept, or with 404;
/// - a body is not the entries JSON;
/// - a page's `next_page` leads to a page already read, or past the page cap;
/// - a page after the first answers 400, which is a broken read and not the
///   token's refusal.
///
/// A 400 on the first page is not an error but
/// [`TrustedPublishers::Refused`]: crates.io refuses a token whose user owns
/// no such crate there.
///
/// Only an API that gave no answer is a [`lookup::Unavailable`], under the
/// conditions of [`RegistryCheck::lists`].
pub fn crates_io_github_configs(base: &str, name: &str, token: &str) -> Result<TrustedPublishers> {
    ensure!(
        !name.is_empty() && name.is_ascii(),
        "{name:?} is not a crates.io crate name"
    );
    let path = format!(
        "{}/trusted_publishing/github_configs",
        base.trim_end_matches('/')
    );
    let mut url = format!("{path}?crate={name}");
    let mut read = HashSet::from([url.clone()]);
    let mut entries = Vec::new();
    loop {
        let body = match github::block(answer(&url, Some(token)))? {
            Answer::Body(body) => body,
            Answer::NotFound => bail!("GET {url}: {}", StatusCode::NOT_FOUND),
            Answer::BadRequest(body) if read.len() == 1 => {
                return Ok(TrustedPublishers::Refused {
                    detail: refusal(&body),
                });
            }
            Answer::BadRequest(body) => {
                bail!("GET {url}: {}: {}", StatusCode::BAD_REQUEST, refusal(&body))
            }
        };
        let page: GithubConfigs = serde_json::from_str(&body)
            .with_context(|| format!("crates.io trusted publishers of {name}"))?;
        if page.github_configs.is_empty() {
            return Ok(TrustedPublishers::Listed(entries));
        }
        entries.extend(page.github_configs);
        let Some(next) = page
            .meta
            .next_page
            .filter(|_| entries.len() < page.meta.total)
        else {
            return Ok(TrustedPublishers::Listed(entries));
        };
        url = format!("{path}{next}");
        ensure!(
            read.insert(url.clone()),
            "crates.io repeated page {url} of the trusted publishers of {name}"
        );
        ensure!(
            read.len() <= MAX_TRUSTED_PUBLISHER_PAGES,
            "crates.io offered more than {MAX_TRUSTED_PUBLISHER_PAGES} pages of the trusted publishers of {name}, the last at {url}"
        );
    }
}

/// What crates.io answered to a request to add a trusted publisher entry.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Added {
    /// 200: the entry exists now.
    Created,
    /// 409: the crate holds the most entries crates.io allows, which is 5.
    AtLimit {
        /// crates.io's reason.
        detail: String,
    },
    /// 400: crates.io refused the entry, as it does for a token whose user
    /// owns no such crate.
    Refused {
        /// crates.io's reason.
        detail: String,
    },
}

/// How often one request for a new entry is sent again after a 429, so one
/// entry takes at most 11 requests.
const MAX_RETRIES: usize = 10;

/// What one request to add an entry came back with.
enum Posted {
    /// An answer that ends the request.
    Answered(Added),
    /// 429, with the `Retry-After` header's text.
    Limited(String),
}

/// Add a GitHub trusted publisher entry for `repository` and `workflow` to
/// `name`, through the crates.io API under `base`, with no environment, so a
/// run in any environment matches it.
///
/// `POST /trusted_publishing/github_configs` takes a token with the
/// `trusted-publishing` endpoint scope, which goes in the `Authorization`
/// header and nowhere else (`create` in crates.io's
/// `src/controllers/trustpub/github_configs/create.rs`). crates.io answers
/// 200, 409 for a crate that holds 5 entries, and 400 for a token whose user
/// does not own the crate.
///
/// On a 429 the call waits until the time `Retry-After` gives, as seconds or
/// as an HTTP date, and sends the request again, at most 10 times.
///
/// # Errors
///
/// Returns an error when:
///
/// - `name` is not ASCII;
/// - `token` cannot be an HTTP header value;
/// - the request fails;
/// - the API answers with a status other than 200, 400, 409, and 429;
/// - a 429 has no `Retry-After` that is seconds or an HTTP date;
/// - the API answers 429 to the request and each of its 10 repeats, which names
///   the last `Retry-After`.
///
/// Only an API that gave no answer is a [`lookup::Unavailable`], under the
/// conditions of [`RegistryCheck::lists`].
pub fn crates_io_add_github_config(
    base: &str,
    name: &str,
    token: &str,
    repository: &Repo,
    workflow: &str,
) -> Result<Added> {
    ensure!(
        !name.is_empty() && name.is_ascii(),
        "{name:?} is not a crates.io crate name"
    );
    let url = format!(
        "{}/trusted_publishing/github_configs",
        base.trim_end_matches('/')
    );
    let body = serde_json::json!({
        "github_config": {
            "crate": name,
            "repository_owner": repository.owner,
            "repository_name": repository.name,
            "workflow_filename": workflow,
            "environment": null,
        }
    })
    .to_string();
    let mut last = String::new();
    for retry in 0..=MAX_RETRIES {
        match github::block(post(&url, token, &body))? {
            Posted::Answered(added) => return Ok(added),
            Posted::Limited(retry_after) => {
                let wait = http_date::wait(&retry_after).with_context(|| {
                    format!(
                        "POST {url}: {}: Retry-After {retry_after:?} is neither seconds nor an HTTP date",
                        StatusCode::TOO_MANY_REQUESTS
                    )
                })?;
                if retry < MAX_RETRIES {
                    thread::sleep(wait);
                }
                last = retry_after;
            }
        }
    }
    bail!(
        "POST {url}: {}: crates.io limited the request {} times; the last Retry-After was {last}",
        StatusCode::TOO_MANY_REQUESTS,
        MAX_RETRIES + 1
    )
}

/// One request to add an entry, with `body` as its JSON.
async fn post(url: &str, token: &str, body: &str) -> Result<Posted> {
    let response = client()?
        .post(url)
        .header(AUTHORIZATION, sensitive(token)?)
        .header(CONTENT_TYPE, "application/json")
        .body(body.to_owned())
        .send()
        .await
        .map_err(|error| unanswered(error, url))?;
    let status = response.status();
    if status == StatusCode::TOO_MANY_REQUESTS {
        let header = response
            .headers()
            .get(RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .with_context(|| format!("POST {url}: {status} with no Retry-After header"))?;
        return Ok(Posted::Limited(header.to_owned()));
    }
    if lookup::no_answer(status, Some(response.headers())) {
        return Err(lookup::unavailable(anyhow::anyhow!("POST {url}: {status}")));
    }
    let outcome = match status {
        StatusCode::OK => return Ok(Posted::Answered(Added::Created)),
        StatusCode::CONFLICT | StatusCode::BAD_REQUEST => status,
        other => bail!("POST {url}: {other}"),
    };
    let text = response
        .text()
        .await
        .map_err(|error| unanswered(error, url))?;
    let detail = Some(reason(&text))
        .filter(|found| !found.is_empty())
        .unwrap_or_else(|| outcome.to_string());
    Ok(Posted::Answered(if outcome == StatusCode::CONFLICT {
        Added::AtLimit { detail }
    } else {
        Added::Refused { detail }
    }))
}

/// The reason in a crates.io error response, `{"errors":[{"detail":"..."}]}`:
/// each non-blank `detail`, trimmed, joined by `; `. A body of another shape is
/// its own reason, trimmed, and an empty one reads as the status, so the
/// message never names an empty reason.
fn refusal(body: &str) -> String {
    let reason = reason(body);
    if reason.is_empty() {
        StatusCode::BAD_REQUEST.to_string()
    } else {
        reason
    }
}

/// The reason in a crates.io error response, as [`refusal`] reads it, or an
/// empty string when the response gives none.
fn reason(body: &str) -> String {
    serde_json::from_str::<ApiErrors>(body).map_or_else(
        |_| body.trim().to_owned(),
        |parsed| {
            parsed
                .errors
                .iter()
                .map(|error| error.detail.trim())
                .filter(|detail| !detail.is_empty())
                .collect::<Vec<_>>()
                .join("; ")
        },
    )
}

/// The crates.io owners response.
#[derive(serde::Deserialize)]
struct Owners {
    users: Vec<Owner>,
}

/// One owner in the owners response.
#[derive(serde::Deserialize)]
struct Owner {
    login: String,
    /// `user` or `team`.
    kind: String,
}

/// The crates.io versions response, newest first under `sort=date`.
#[derive(serde::Deserialize)]
struct Versions {
    versions: Vec<VersionData>,
}

/// One version in the versions response.
#[derive(serde::Deserialize)]
struct VersionData {
    /// `null` for a version not published through trusted publishing.
    #[serde(default)]
    trustpub_data: Option<TrustpubData>,
}

/// A version's `trustpub_data`, tagged by its `provider`.
#[derive(serde::Deserialize)]
#[serde(tag = "provider", rename_all = "lowercase")]
enum TrustpubData {
    Github { repository: String },
    Gitlab { project_path: String },
}

/// One page of the GitHub trusted publisher entries response.
#[derive(serde::Deserialize)]
struct GithubConfigs {
    github_configs: Vec<GithubConfig>,
    meta: GithubConfigsMeta,
}

/// The paging part of the entries response.
#[derive(serde::Deserialize)]
struct GithubConfigsMeta {
    /// The query string of the next page, or `None` on the last.
    next_page: Option<String>,
    /// How many entries the crate has over every page.
    total: usize,
}

/// A crates.io error response.
#[derive(serde::Deserialize)]
struct ApiErrors {
    errors: Vec<ApiError>,
}

/// One error in a crates.io error response.
#[derive(serde::Deserialize)]
struct ApiError {
    detail: String,
}

/// One line of a crate's index file.
#[derive(serde::Deserialize)]
struct IndexRecord {
    vers: String,
    #[serde(default)]
    yanked: bool,
}

/// What the server at a URL answered.
enum Answer {
    /// 200, with its body.
    Body(String),
    /// 404, which the index and the API both answer for a crate they do not
    /// have.
    NotFound,
    /// 400, with its body: crates.io's refusal of a token for a crate its user
    /// does not own.
    BadRequest(String),
}

/// The body at `url`, or `None` on a 404, which the index and the API both
/// answer for a crate they do not have. A 400 is an error here: only the
/// trusted publishers request reads it as a refusal.
async fn fetch(url: &str, token: Option<&str>) -> Result<Option<String>> {
    match answer(url, token).await? {
        Answer::Body(body) => Ok(Some(body)),
        Answer::NotFound => Ok(None),
        Answer::BadRequest(_) => bail!("GET {url}: {}", StatusCode::BAD_REQUEST),
    }
}

/// What the server at `url` answered. With `token`, the request carries it as
/// its `Authorization` header, the raw token as cargo sends it, marked
/// sensitive so no debug output shows it. Any status but 200, 400, and 404 is
/// an error, a [`lookup::Unavailable`] when the server gave no answer.
async fn answer(url: &str, token: Option<&str>) -> Result<Answer> {
    let mut request = client()?.get(url);
    if let Some(token) = token {
        request = request.header(AUTHORIZATION, sensitive(token)?);
    }
    let response = request
        .send()
        .await
        .map_err(|error| unanswered(error, url))?;
    match response.status() {
        StatusCode::OK => Ok(Answer::Body(
            response
                .text()
                .await
                .map_err(|error| unanswered(error, url))?,
        )),
        StatusCode::NOT_FOUND => Ok(Answer::NotFound),
        StatusCode::BAD_REQUEST => Ok(Answer::BadRequest(
            response
                .text()
                .await
                .map_err(|error| unanswered(error, url))?,
        )),
        status if lookup::no_answer(status, Some(response.headers())) => {
            Err(lookup::unavailable(anyhow::anyhow!("GET {url}: {status}")))
        }
        status => bail!("GET {url}: {status}"),
    }
}

/// The HTTP client every crates.io request uses.
fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(user_agent())
        .timeout(INDEX_TIMEOUT)
        .build()
        .context("HTTP client")
}

/// `token` as an `Authorization` value, the raw token as cargo sends it,
/// marked sensitive so no debug output shows it.
fn sensitive(token: &str) -> Result<HeaderValue> {
    let mut value = HeaderValue::from_str(token)
        .context("the crates.io token holds a character an HTTP header cannot carry")?;
    value.set_sensitive(true);
    Ok(value)
}

/// `error` from a request to `url`, marked as a [`lookup::Unavailable`] when
/// the index gave no answer. The request could not connect, which covers DNS,
/// or it timed out. Or the body broke off while it was read: a reset, or a
/// timeout reqwest 0.13 reports as a body error.
fn unanswered(error: reqwest::Error, url: &str) -> anyhow::Error {
    let no_answer = error.is_connect() || error.is_timeout() || error.is_body();
    let error = anyhow::Error::new(error).context(format!("GET {url}"));
    if no_answer {
        lookup::unavailable(error)
    } else {
        error
    }
}

/// Automated clients must identify themselves to crates.io, as its
/// [data access policy] requires.
///
/// [data access policy]: https://crates.io/data-access
fn user_agent() -> String {
    format!(
        "verctl/{} (+{})",
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_REPOSITORY")
    )
}

/// Run a `query` in `dir`: listed when it exits zero and one of its stdout
/// lines, trimmed of whitespace and quotes, is `version`.
fn query_lists(argv: &[String], dir: &Path, version: &str, env: &[(&str, &str)]) -> Result<bool> {
    match process::run_in(argv, dir, env, QUERY_TIMEOUT) {
        Ok(stdout) => Ok(stdout
            .lines()
            .any(|line| line.trim().trim_matches(['"', '\'']) == version)),
        Err(error) if is_exit_failure(&error) => Ok(false),
        Err(error) if error.is::<process::TimedOut>() => Err(lookup::unavailable(error)),
        Err(error) => Err(error),
    }
}

/// Whether an error from [`process::run_in`] is the command exiting non-zero.
/// A `query` uses that exit for "not listed". A failure to run it is another
/// case.
fn is_exit_failure(error: &anyhow::Error) -> bool {
    error.downcast_ref::<process::ExitFailure>().is_some()
}

/// Ask `check` until it lists `version`. Pause between asks, using each entry
/// of `delays` in turn.
/// A crate is read from `index`, and a `query` runs with `env` set, as
/// [`RegistryCheck::lists`] describes.
///
/// The publish already happened. A failed check (an index timeout, a 5xx)
/// therefore counts as "not listed yet" for that round and does not fail the
/// release. Only the last round's error is reported.
///
/// # Errors
///
/// Returns an error when the registry still does not list `version` after the
/// last pause, carrying the last round's check error when it had one.
pub fn wait_listed(
    check: &RegistryCheck,
    index: &str,
    coord: &str,
    version: &str,
    env: &[(&str, &str)],
    delays: &[Duration],
) -> Result<()> {
    let mut last_error = None;
    for delay in delays {
        thread::sleep(*delay);
        match check.lists(index, version, env) {
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
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use indoc::{formatdoc, indoc};
    use serde_json::{Value, json};
    use tempfile::TempDir;
    use tokio::task;
    use wiremock::matchers::{
        body_json, header, method, path, query_param, query_param_is_missing,
    };
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    use super::{
        Added, CRATES_IO_INDEX, CargoEnv, CargoTarget, MAX_RETRIES, MAX_TRUSTED_PUBLISHER_PAGES,
        RegistryCheck, TrustedPublishers, TrustedUpload, cargo_target, crates_io_add_github_config,
        crates_io_baseline, crates_io_github_configs, crates_io_lists, crates_io_newest_trust,
        crates_io_user_owners, index_path, refusal, wait_listed,
    };
    use crate::{github, lookup};

    #[test]
    fn index_paths_follow_the_cargo_layout() {
        assert_eq!(index_path("a"), "1/a");
        assert_eq!(index_path("ab"), "2/ab");
        assert_eq!(index_path("abc"), "3/a/abc");
        assert_eq!(index_path("serde"), "se/rd/serde");
        assert_eq!(index_path("Inflector"), "in/fl/inflector");
    }

    async fn lists(body: Option<&str>, name: &str, version: &str) -> bool {
        let response = body.map_or_else(
            || ResponseTemplate::new(404),
            |body| ResponseTemplate::new(200).set_body_string(body),
        );
        index_answers(response, name, version).await.unwrap()
    }

    /// What `crates_io_lists` makes of an index that answers `response` for
    /// `name`.
    async fn index_answers(
        response: ResponseTemplate,
        name: &str,
        version: &str,
    ) -> anyhow::Result<bool> {
        let server = MockServer::start().await;
        let file = format!("/{}", index_path(name));
        Mock::given(method("GET"))
            .and(path(file))
            .respond_with(response)
            .mount(&server)
            .await;
        let base = server.uri();
        let name = name.to_owned();
        let version = version.to_owned();
        task::spawn_blocking(move || crates_io_lists(&base, &name, &version))
            .await
            .unwrap()
    }

    /// A rate limit or a 5xx is no answer, so `status` reports the check as
    /// not run. A plain 403 is a refusal and stays an error.
    #[tokio::test]
    async fn an_index_that_cannot_answer_now_is_a_lookup_that_cannot_run() {
        for response in [
            ResponseTemplate::new(429),
            ResponseTemplate::new(403).insert_header("x-ratelimit-remaining", "0"),
            ResponseTemplate::new(503),
        ] {
            let error = index_answers(response, "demo", "0.4.5").await.unwrap_err();
            assert!(lookup::is_unavailable(&error), "{error:#}");
        }
        let error = index_answers(ResponseTemplate::new(403), "demo", "0.4.5")
            .await
            .unwrap_err();
        assert!(!lookup::is_unavailable(&error), "{error:#}");
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
        task::spawn_blocking(move || crates_io_baseline(&base, "demo", &current))
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

    /// What `crates_io_user_owners` makes of an API that answers `response`
    /// for `demo`'s owners.
    async fn owners(response: ResponseTemplate) -> Option<Vec<String>> {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crates/demo/owners"))
            .respond_with(response)
            .mount(&server)
            .await;
        let base = server.uri();
        task::spawn_blocking(move || crates_io_user_owners(&base, "demo"))
            .await
            .unwrap()
            .unwrap()
    }

    const OWNERS: &str = indoc! {r#"
        {"users":[
          {"id":1,"login":"any-victor","kind":"user","url":"https://github.com/any-victor","name":"Victor"},
          {"id":2,"login":"github:victor-software-house:owners","kind":"team","url":"https://github.com/orgs/victor-software-house/teams/owners","name":"owners"},
          {"id":3,"login":"other","kind":"user","url":"https://github.com/other","name":null}
        ]}
    "#};

    #[tokio::test]
    async fn the_owners_of_a_crate_are_its_user_logins() {
        assert_eq!(
            owners(ResponseTemplate::new(200).set_body_string(OWNERS)).await,
            Some(vec!["any-victor".to_owned(), "other".to_owned()])
        );
    }

    /// A 404 is a crate crates.io does not know, which `check registry` reads
    /// apart from a crate with no user owner.
    #[tokio::test]
    async fn a_crate_crates_io_does_not_know_has_no_owners() {
        assert_eq!(owners(ResponseTemplate::new(404)).await, None);
    }

    /// What `crates_io_newest_trust` makes of a versions answer of `body`.
    async fn newest_trust(body: &str) -> TrustedUpload {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crates/demo/versions"))
            .and(query_param("per_page", "1"))
            .and(query_param("sort", "date"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;
        let base = server.uri();
        task::spawn_blocking(move || crates_io_newest_trust(&base, "demo"))
            .await
            .unwrap()
            .unwrap()
    }

    /// The newest version's `trustpub_data` names its provider: a GitHub
    /// repository, a GitLab project, or `null` for a token upload.
    #[tokio::test]
    async fn the_newest_version_shows_how_it_was_published() {
        assert_eq!(
            newest_trust(indoc! {r#"
                {"versions":[{"num":"0.7.5","trustpub_data":{"provider":"github","repository":"victor-software-house/ctl-core","run_id":"43479037333","sha":"2af7"}}],"meta":{"total":12,"next_page":null}}
            "#})
            .await,
            TrustedUpload::GitHub {
                repository: "victor-software-house/ctl-core".into()
            }
        );
        assert_eq!(
            newest_trust(indoc! {r#"
                {"versions":[{"num":"1.0.0","trustpub_data":{"provider":"gitlab","project_path":"group/demo","job_id":"7","sha":"abc"}}],"meta":{"total":1,"next_page":null}}
            "#})
            .await,
            TrustedUpload::GitLab {
                project_path: "group/demo".into()
            }
        );
        assert_eq!(
            newest_trust(indoc! {r#"
                {"versions":[{"num":"0.1.0","trustpub_data":null}],"meta":{"total":1,"next_page":null}}
            "#})
            .await,
            TrustedUpload::Untrusted
        );
    }

    /// The trusted publisher entries over two pages, each request carrying
    /// the raw token as its `Authorization` header.
    #[tokio::test]
    async fn the_trusted_publishers_of_a_crate_follow_every_page() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/trusted_publishing/github_configs"))
            .and(query_param("crate", "demo"))
            .and(query_param("seek", "abc"))
            .and(header("authorization", "t0ken"))
            .respond_with(ResponseTemplate::new(200).set_body_string(indoc! {r#"
                {"github_configs":[{"id":2,"crate":"demo","repository_owner":"acme","repository_name":"demo","repository_owner_id":9,"workflow_filename":"self-release.yml","environment":"release","created_at":"2026-11-13T00:00:00Z"}],"meta":{"total":2,"next_page":null}}
            "#}))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/trusted_publishing/github_configs"))
            .and(query_param("crate", "demo"))
            .and(query_param_is_missing("seek"))
            .and(header("authorization", "t0ken"))
            .respond_with(ResponseTemplate::new(200).set_body_string(indoc! {r#"
                {"github_configs":[{"id":1,"crate":"demo","repository_owner":"acme","repository_name":"demo","repository_owner_id":9,"workflow_filename":"release.yml","environment":null,"created_at":"2026-11-13T00:00:00Z"}],"meta":{"total":2,"next_page":"?crate=demo&seek=abc"}}
            "#}))
            .mount(&server)
            .await;
        let base = server.uri();
        let TrustedPublishers::Listed(entries) =
            task::spawn_blocking(move || crates_io_github_configs(&base, "demo", "t0ken"))
                .await
                .unwrap()
                .unwrap()
        else {
            panic!("crates.io refused the token");
        };
        let files: Vec<(&str, Option<&str>)> = entries
            .iter()
            .map(|entry| {
                (
                    entry.workflow_filename.as_str(),
                    entry.environment.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            files,
            [("release.yml", None), ("self-release.yml", Some("release"))]
        );
        assert!(
            entries
                .iter()
                .all(|entry| entry.repository_owner == "acme" && entry.repository_name == "demo")
        );
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 2);
    }

    /// `crates_io_github_configs` for `demo` against `base`, with the token
    /// `t0ken`, off the async runtime as the command runs it.
    async fn publishers_of_demo(base: String) -> anyhow::Result<TrustedPublishers> {
        task::spawn_blocking(move || crates_io_github_configs(&base, "demo", "t0ken"))
            .await
            .unwrap()
    }

    /// A page with no entries ends the paging, whatever its `next_page` says,
    /// so a server that keeps offering a next page cannot hold the check.
    #[tokio::test]
    async fn a_page_with_no_entries_stops_the_paging() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/trusted_publishing/github_configs"))
            .and(query_param("seek", "abc"))
            .respond_with(ResponseTemplate::new(200).set_body_string(indoc! {r#"
                {"github_configs":[],"meta":{"total":1,"next_page":"?crate=demo&seek=def"}}
            "#}))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/trusted_publishing/github_configs"))
            .and(query_param_is_missing("seek"))
            .respond_with(ResponseTemplate::new(200).set_body_string(indoc! {r#"
                {"github_configs":[{"id":1,"crate":"demo","repository_owner":"acme","repository_name":"demo","repository_owner_id":9,"workflow_filename":"release.yml","environment":null,"created_at":"2026-11-13T00:00:00Z"}],"meta":{"total":2,"next_page":"?crate=demo&seek=abc"}}
            "#}))
            .mount(&server)
            .await;
        let TrustedPublishers::Listed(entries) = publishers_of_demo(server.uri()).await.unwrap()
        else {
            panic!("crates.io refused the token");
        };
        assert_eq!(entries.len(), 1);
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 2);
    }

    /// A `next_page` that leads back to a page already read fails, naming the
    /// crate and the repeated URL.
    #[tokio::test]
    async fn a_next_page_that_repeats_a_page_fails() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/trusted_publishing/github_configs"))
            .respond_with(ResponseTemplate::new(200).set_body_string(indoc! {r#"
                {"github_configs":[{"id":1,"crate":"demo","repository_owner":"acme","repository_name":"demo","repository_owner_id":9,"workflow_filename":"release.yml","environment":null,"created_at":"2026-11-13T00:00:00Z"}],"meta":{"total":5,"next_page":"?crate=demo"}}
            "#}))
            .mount(&server)
            .await;
        let base = server.uri();
        let error = publishers_of_demo(base.clone()).await.unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "crates.io repeated page {base}/trusted_publishing/github_configs?crate=demo of the trusted publishers of demo"
            )
        );
    }

    /// Once the entries reach `meta.total`, the paging ends, whatever
    /// `next_page` says.
    #[tokio::test]
    async fn the_paging_ends_at_the_total() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/trusted_publishing/github_configs"))
            .respond_with(ResponseTemplate::new(200).set_body_string(indoc! {r#"
                {"github_configs":[{"id":1,"crate":"demo","repository_owner":"acme","repository_name":"demo","repository_owner_id":9,"workflow_filename":"release.yml","environment":null,"created_at":"2026-11-13T00:00:00Z"}],"meta":{"total":1,"next_page":"?crate=demo&seek=abc"}}
            "#}))
            .mount(&server)
            .await;
        let TrustedPublishers::Listed(entries) = publishers_of_demo(server.uri()).await.unwrap()
        else {
            panic!("crates.io refused the token");
        };
        assert_eq!(entries.len(), 1);
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    /// A server that keeps offering new pages stops the read after
    /// [`MAX_TRUSTED_PUBLISHER_PAGES`], naming the crate and the last URL.
    #[tokio::test]
    async fn endless_new_pages_stop_at_the_cap() {
        struct NextSeek(AtomicUsize);
        impl Respond for NextSeek {
            fn respond(&self, _: &Request) -> ResponseTemplate {
                let seek = self.0.fetch_add(1, Ordering::SeqCst);
                ResponseTemplate::new(200).set_body_string(formatdoc! {r#"
                    {{"github_configs":[{{"id":1,"crate":"demo","repository_owner":"acme","repository_name":"demo","repository_owner_id":9,"workflow_filename":"ci.yml","environment":null,"created_at":"2026-11-13T00:00:00Z"}}],"meta":{{"total":100000,"next_page":"?crate=demo&seek={seek}"}}}}
                "#})
            }
        }
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/trusted_publishing/github_configs"))
            .respond_with(NextSeek(AtomicUsize::new(0)))
            .mount(&server)
            .await;
        let error = publishers_of_demo(server.uri()).await.unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("crates.io offered more than 100 pages of the trusted publishers of demo, the last at "),
            "{error}"
        );
        assert_eq!(
            server.received_requests().await.unwrap().len(),
            MAX_TRUSTED_PUBLISHER_PAGES
        );
    }

    /// A 400 on a later page is not the token's refusal but a failed read:
    /// it fails naming the URL, and drops no entry silently.
    #[tokio::test]
    async fn a_400_on_a_later_page_fails_naming_it() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/trusted_publishing/github_configs"))
            .and(query_param("seek", "abc"))
            .respond_with(ResponseTemplate::new(400).set_body_string(indoc! {r#"
                {"errors":[{"detail":"invalid seek"}]}
            "#}))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/trusted_publishing/github_configs"))
            .and(query_param_is_missing("seek"))
            .respond_with(ResponseTemplate::new(200).set_body_string(indoc! {r#"
                {"github_configs":[{"id":1,"crate":"demo","repository_owner":"acme","repository_name":"demo","repository_owner_id":9,"workflow_filename":"release.yml","environment":null,"created_at":"2026-11-13T00:00:00Z"}],"meta":{"total":2,"next_page":"?crate=demo&seek=abc"}}
            "#}))
            .mount(&server)
            .await;
        let base = server.uri();
        let error = publishers_of_demo(base.clone()).await.unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "GET {base}/trusted_publishing/github_configs?crate=demo&seek=abc: 400 Bad Request: invalid seek"
            )
        );
    }

    /// The body crates.io's create endpoint takes for `demo`, `acme/demo`,
    /// and `release.yml`, with no environment.
    fn entry_body() -> Value {
        json!({
            "github_config": {
                "crate": "demo",
                "repository_owner": "acme",
                "repository_name": "demo",
                "workflow_filename": "release.yml",
                "environment": null,
            }
        })
    }

    /// Add the entry to `demo` through the API at `server`, off the async
    /// runtime as the command runs it.
    async fn add_entry(server: &MockServer) -> anyhow::Result<Added> {
        let base = server.uri();
        task::spawn_blocking(move || {
            crates_io_add_github_config(
                &base,
                "demo",
                "t0ken",
                &github::parse_slug("acme/demo").unwrap(),
                "release.yml",
            )
        })
        .await
        .unwrap()
    }

    /// `POST /trusted_publishing/github_configs` with the token and the body
    /// above, answering `response`, at most `times` times.
    async fn accepting(server: &MockServer, response: ResponseTemplate, times: Option<u64>) {
        let mock = Mock::given(method("POST"))
            .and(path("/trusted_publishing/github_configs"))
            .and(header("authorization", "t0ken"))
            .and(body_json(entry_body()))
            .respond_with(response);
        match times {
            Some(times) => mock.up_to_n_times(times).mount(server).await,
            None => mock.mount(server).await,
        }
    }

    fn limited(retry_after: &str) -> ResponseTemplate {
        ResponseTemplate::new(429).insert_header("Retry-After", retry_after)
    }

    /// A 200 adds the entry, and the one request carries the exact JSON
    /// body, the raw token, and no other header that holds it.
    #[tokio::test]
    async fn a_200_adds_the_entry() {
        let server = MockServer::start().await;
        accepting(
            &server,
            ResponseTemplate::new(200).set_body_json(json!({
                "github_config": {
                    "id": 1,
                    "crate": "demo",
                    "repository_owner": "acme",
                    "repository_owner_id": 9,
                    "repository_name": "demo",
                    "workflow_filename": "release.yml",
                    "environment": null,
                    "created_at": "2026-11-13T00:00:00Z",
                }
            })),
            None,
        )
        .await;
        assert_eq!(add_entry(&server).await.unwrap(), Added::Created);
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            serde_json::from_slice::<Value>(&requests[0].body).unwrap(),
            entry_body()
        );
        assert_eq!(
            requests[0].headers.get("content-type").unwrap(),
            "application/json"
        );
    }

    /// A 409 is the crate at its limit of 5 entries, with crates.io's reason.
    #[tokio::test]
    async fn a_409_is_the_limit() {
        let server = MockServer::start().await;
        accepting(
            &server,
            ResponseTemplate::new(409).set_body_json(json!({
                "errors": [{ "detail": "This crate already has the maximum of 5 trusted publishers" }],
            })),
            None,
        )
        .await;
        assert_eq!(
            add_entry(&server).await.unwrap(),
            Added::AtLimit {
                detail: "This crate already has the maximum of 5 trusted publishers".into()
            }
        );
    }

    /// A 400 is a refusal with crates.io's reason, and an empty 409 reads as
    /// its status.
    #[tokio::test]
    async fn a_400_is_a_refusal() {
        let server = MockServer::start().await;
        accepting(
            &server,
            ResponseTemplate::new(400).set_body_json(json!({
                "errors": [{ "detail": "You are not an owner of this crate" }],
            })),
            None,
        )
        .await;
        assert_eq!(
            add_entry(&server).await.unwrap(),
            Added::Refused {
                detail: "You are not an owner of this crate".into()
            }
        );
        let server = MockServer::start().await;
        accepting(&server, ResponseTemplate::new(409), None).await;
        assert_eq!(
            add_entry(&server).await.unwrap(),
            Added::AtLimit {
                detail: "409 Conflict".into()
            }
        );
    }

    /// A 429 with `Retry-After: 0` is sent again at once, and the second
    /// request adds the entry.
    #[tokio::test]
    async fn a_429_with_zero_seconds_is_sent_again() {
        let server = MockServer::start().await;
        accepting(&server, limited("0"), Some(1)).await;
        accepting(&server, ResponseTemplate::new(200), None).await;
        assert_eq!(add_entry(&server).await.unwrap(), Added::Created);
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }

    /// A 429 whose `Retry-After` is an HTTP date already past waits no time.
    #[tokio::test]
    async fn a_429_with_a_past_date_is_sent_again() {
        let server = MockServer::start().await;
        accepting(&server, limited("Fri, 13 Nov 2026 15:10:00 GMT"), Some(1)).await;
        accepting(&server, ResponseTemplate::new(200), None).await;
        assert_eq!(add_entry(&server).await.unwrap(), Added::Created);
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }

    /// The request and its 10 repeats all answered 429 fail, naming the last
    /// `Retry-After`.
    #[tokio::test]
    async fn eleven_429s_fail_naming_the_last_retry_after() {
        let server = MockServer::start().await;
        accepting(&server, limited("0"), None).await;
        let error = add_entry(&server).await.unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("429 Too Many Requests"), "{message}");
        assert!(message.contains("the last Retry-After was 0"), "{message}");
        assert_eq!(
            server.received_requests().await.unwrap().len(),
            MAX_RETRIES + 1
        );
    }

    /// A 429 with no usable `Retry-After` fails at once.
    #[tokio::test]
    async fn a_429_without_a_retry_after_fails() {
        let server = MockServer::start().await;
        accepting(&server, ResponseTemplate::new(429), None).await;
        let message = format!("{:#}", add_entry(&server).await.unwrap_err());
        assert!(message.contains("no Retry-After header"), "{message}");
        let server = MockServer::start().await;
        accepting(&server, limited("soon"), None).await;
        let message = format!("{:#}", add_entry(&server).await.unwrap_err());
        assert!(
            message.contains("neither seconds nor an HTTP date"),
            "{message}"
        );
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    /// Error details that are all blank read as the status.
    #[test]
    fn blank_details_read_as_the_status() {
        assert_eq!(
            refusal(r#"{"errors":[{"detail":""},{"detail":"  "}]}"#),
            "400 Bad Request"
        );
    }

    /// A 400 is crates.io refusing the token for a crate its user does not
    /// own: the detail comes back, and the token is nowhere in it.
    #[tokio::test]
    async fn a_400_is_a_refusal_with_crates_ios_detail() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/trusted_publishing/github_configs"))
            .and(header("authorization", "t0ken"))
            .respond_with(ResponseTemplate::new(400).set_body_string(indoc! {r#"
                {"errors":[{"detail":"You are not an owner of this crate"}]}
            "#}))
            .mount(&server)
            .await;
        assert_eq!(
            publishers_of_demo(server.uri()).await.unwrap(),
            TrustedPublishers::Refused {
                detail: "You are not an owner of this crate".into()
            }
        );
    }

    /// A 400 with an empty body reads as its status.
    #[tokio::test]
    async fn an_empty_400_reads_as_its_status() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/trusted_publishing/github_configs"))
            .respond_with(ResponseTemplate::new(400))
            .mount(&server)
            .await;
        assert_eq!(
            publishers_of_demo(server.uri()).await.unwrap(),
            TrustedPublishers::Refused {
                detail: "400 Bad Request".into()
            }
        );
    }

    #[tokio::test]
    async fn an_owners_api_that_cannot_answer_now_is_a_lookup_that_cannot_run() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crates/demo/owners"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let base = server.uri();
        let error = task::spawn_blocking(move || crates_io_user_owners(&base, "demo"))
            .await
            .unwrap()
            .unwrap_err();
        assert!(lookup::is_unavailable(&error), "{error:#}");
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

    fn write_config(dir: &Path, name: &str, body: &str) {
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
        let error = wait_listed(
            &check,
            CRATES_IO_INDEX,
            "demo",
            "1.0.0",
            &[],
            &[Duration::ZERO, Duration::ZERO],
        )
        .unwrap_err();
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
        let error = wait_listed(
            &check,
            CRATES_IO_INDEX,
            "demo",
            "1.0.0",
            &[],
            &[Duration::ZERO],
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "demo: the publisher exited zero, but the registry does not list 1.0.0"
        );
    }
}
