//! How a package is published.
//!
//! [`Publisher::of`] resolves it once: the built-in cargo or bun upload, a
//! command the repository wrote, or a release on GitHub only. A built-in
//! publisher has typed options a package sets under `publish`, such as
//! [`CargoPublish::verify`], and keeps its registry check. A replaced
//! command has none of either.
//!
//! Placeholders in a `publishers` entry: `{path}` `{dir}` `{registry}`
//! `{config}`, and in a `query` also `{name}` `{version}`. A placeholder with
//! no value fails the run instead of dropping its argument.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;

use crate::config::PublishOptions;
use crate::members;
use crate::registry::{self, CargoEnv, CargoTarget, RegistryCheck};

/// A publisher recipe: the command to upload a package and where its registry
/// config lives.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct PublisherSpec {
    /// What the registry calls the package. Defaults to `package`.
    pub noun: Option<String>,
    /// Upload command, with the placeholders from the module docs.
    pub argv: Option<Vec<String>>,
    /// Ancestor file that owns registry/auth (`bunfig.toml`).
    pub config_name: Option<String>,
    /// Arguments appended when `config_name` is found.
    pub config_argv: Option<Vec<String>>,
    /// Appended when `registry` is set and does not alias to empty.
    pub registry_argv: Option<Vec<String>>,
    /// Appended when there is no registry (or the alias is empty).
    pub default_argv: Option<Vec<String>>,
    /// Registry aliases. An empty value means the default registry, which needs
    /// no flag.
    #[serde(default)]
    pub registries: BTreeMap<String, String>,
    /// Asks the registry whether a version is published: it exits zero and
    /// prints the version when the registry has it. It runs in the package
    /// directory, where the publisher's registry config is in effect. When
    /// omitted, the stock recipe's query applies; an empty list turns the check
    /// off, leaving the stderr match.
    pub query: Option<Vec<String>>,
}

impl PublisherSpec {
    fn stock(name: &str) -> Option<Self> {
        match name {
            "cargo" => Some(Self {
                noun: Some("crate".into()),
                argv: Some(vec![
                    "cargo".into(),
                    "publish".into(),
                    "--locked".into(),
                    "--manifest-path".into(),
                    "{path}".into(),
                ]),
                registry_argv: Some(vec!["--registry".into(), "{registry}".into()]),
                registries: BTreeMap::from([("crates-io".into(), String::new())]),
                ..Self::default()
            }),
            "bun" => Some(Self {
                noun: Some("package".into()),
                argv: Some(vec![
                    "bun".into(),
                    "publish".into(),
                    "--tolerate-republish".into(),
                    "--cwd".into(),
                    "{dir}".into(),
                ]),
                config_name: Some("bunfig.toml".into()),
                config_argv: Some(vec!["--config".into(), "{config}".into()]),
                registry_argv: Some(vec!["--registry".into(), "{registry}".into()]),
                default_argv: Some(vec!["--access".into(), "public".into()]),
                registries: BTreeMap::from([
                    ("npm".into(), String::new()),
                    ("github".into(), "https://npm.pkg.github.com".into()),
                ]),
                query: Some(BUN_QUERY.iter().map(|part| (*part).to_owned()).collect()),
            }),
            _ => None,
        }
    }

    fn merge(self, over: Self) -> Self {
        Self {
            noun: over.noun.or(self.noun),
            argv: over.argv.or(self.argv),
            config_name: over.config_name.or(self.config_name),
            config_argv: over.config_argv.or(self.config_argv),
            registry_argv: over.registry_argv.or(self.registry_argv),
            default_argv: over.default_argv.or(self.default_argv),
            registries: if over.registries.is_empty() {
                self.registries
            } else {
                over.registries
            },
            query: over.query.or(self.query),
        }
    }

    /// Render the noun and argv for one package, expanding placeholders.
    ///
    /// # Errors
    ///
    /// Returns an error when the publisher declares no `argv`, or when an
    /// argument uses a placeholder with no value for this package.
    pub fn render(
        &self,
        name: &str,
        manifest: &Path,
        registry: Option<&str>,
    ) -> Result<(String, Vec<String>)> {
        let argv = self
            .argv
            .as_ref()
            .with_context(|| format!("publisher {name:?} needs argv"))?;
        let out = self.assemble(argv, manifest, registry, &[], Appended::Upload)?;
        let noun = self.noun.clone().unwrap_or_else(|| "package".into());
        Ok((noun, out))
    }

    /// Expand `argv` for the package in `manifest`, then append what
    /// `appended` asks for. `extra` adds placeholders.
    fn assemble(
        &self,
        argv: &[String],
        manifest: &Path,
        registry: Option<&str>,
        extra: &[(&'static str, String)],
        appended: Appended,
    ) -> Result<Vec<String>> {
        let dir = package_dir(manifest);
        let config = self
            .config_name
            .as_deref()
            .and_then(|file| find_config(manifest, file));
        let registry = resolve_registry(&self.registries, registry);
        let mut ctx = BTreeMap::from([
            ("path", manifest.display().to_string()),
            ("dir", dir.display().to_string()),
        ]);
        ctx.extend(extra.iter().cloned());
        if let Some(path) = &config {
            ctx.insert("config", path.display().to_string());
        }
        if let Some(registry) = &registry {
            ctx.insert("registry", registry.clone());
        }
        let mut out = expand(argv, &ctx)?;
        let (recipe, default) = match appended {
            Appended::Upload => (true, true),
            Appended::StockQuery => (true, false),
            Appended::Nothing => (false, false),
        };
        if recipe
            && config.is_some()
            && let Some(extra) = &self.config_argv
        {
            out.extend(expand(extra, &ctx)?);
        }
        if recipe && config.is_none() && registry.is_some() {
            if let Some(extra) = &self.registry_argv {
                out.extend(expand(extra, &ctx)?);
            }
        } else if default
            && registry.is_none()
            && let Some(extra) = &self.default_argv
        {
            out.extend(expand(extra, &ctx)?);
        }
        Ok(out)
    }
}

/// What [`PublisherSpec::assemble`] appends after the expanded argv.
#[derive(Debug, Clone, Copy)]
enum Appended {
    /// The recipe's config or registry arguments, else `default_argv`
    /// (`--access public`): the full upload command.
    Upload,
    /// The recipe's config or registry arguments, so a stock query targets the
    /// registry the upload goes to.
    StockQuery,
    /// Nothing: a query a repository declares runs as written, and reaches
    /// the registry through its own `{registry}` or `{config}` placeholder.
    Nothing,
}

/// The directory a manifest sits in; `.` for a bare file name.
fn package_dir(manifest: &Path) -> &Path {
    manifest
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn resolve_registry(aliases: &BTreeMap<String, String>, registry: Option<&str>) -> Option<String> {
    let raw = registry?.trim();
    if raw.is_empty() {
        return None;
    }
    match aliases.get(raw) {
        Some(mapped) if mapped.is_empty() => None,
        Some(mapped) => Some(mapped.clone()),
        None => Some(raw.to_owned()),
    }
}

/// Replace each `{placeholder}` in `parts` with the value `ctx` holds for it.
///
/// A placeholder left after expansion fails the run instead of dropping its
/// argument. A dropped argument would shorten the command line, and a typo
/// such as `{registr}` would surface later as another tool's error.
///
/// A known placeholder with no value for this package, such as `{registry}`
/// on a package with no registry, fails the same way. Text in braces that is
/// not a placeholder name, such as a JSON argument, passes through.
///
/// # Errors
///
/// Returns an error naming the argument, the placeholder, and the
/// placeholders that have a value here.
fn expand(parts: &[String], ctx: &BTreeMap<&str, String>) -> Result<Vec<String>> {
    let mut out = Vec::with_capacity(parts.len());
    for part in parts {
        let mut expanded = part.clone();
        for (key, value) in ctx {
            expanded = expanded.replace(&format!("{{{key}}}"), value);
        }
        if let Some(name) = placeholder_in(&expanded) {
            let known = ctx
                .keys()
                .map(|key| format!("{{{key}}}"))
                .collect::<Vec<_>>()
                .join(" ");
            bail!(
                "argument {part:?} uses {{{name}}}, which has no value for this package; the placeholders with a value are {known}"
            );
        }
        if !expanded.is_empty() {
            out.push(expanded);
        }
    }
    Ok(out)
}

/// The first `{name}` in `text` whose name is a placeholder name: a letter or
/// underscore, then letters, digits, or underscores.
fn placeholder_in(text: &str) -> Option<&str> {
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let close = after.find('}')?;
        let name = &after[..close];
        let mut chars = name.chars();
        if chars
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Some(name);
        }
        rest = after;
    }
    None
}

fn find_config(manifest: &Path, file_name: &str) -> Option<PathBuf> {
    let start = manifest.parent().unwrap_or(manifest);
    for dir in start.ancestors() {
        let candidate = dir.join(file_name);
        if candidate.is_file() {
            return Some(candidate);
        }
        if dir.join(".git").exists() {
            break;
        }
    }
    None
}

/// The stock bun query. `bun pm view` is an alias of `bun info`, which prints
/// one property of the selected version and fails when the version does not
/// exist, as the [`bun info`] page documents.
///
/// [`bun info`]: https://bun.com/docs/pm/cli/info
const BUN_QUERY: [&str; 5] = ["bun", "pm", "view", "{name}@{version}", "version"];

/// The publisher name for a package released only on GitHub: a tag and a
/// Release, with assets when declared, and no registry step. Its argv is
/// empty, and `release` reads that as "nothing to upload". Every other
/// publisher must render a command. An empty argv therefore never passes for
/// this mode.
pub const RELEASE_ONLY: &str = "none";

/// The options of the built-in cargo upload.
///
/// This is the one place that names them, their types, and what each means
/// when omitted. A package writes them under `publish`. They change the
/// built-in command without replacing it, so the crates.io index check and
/// the member-by-member upload of a workspace stay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct CargoPublish {
    /// Whether `cargo publish` builds the packed crate before it uploads.
    /// `false` adds `--no-verify`. Set it when CI already builds the packed
    /// crate of every commit the default branch receives, such as a
    /// `cargo package` step the merge queue requires. When omitted, cargo
    /// verifies, as the [`cargo publish`] page describes.
    ///
    /// [`cargo publish`]: https://doc.rust-lang.org/cargo/commands/cargo-publish.html
    pub verify: bool,
}

impl Default for CargoPublish {
    fn default() -> Self {
        Self { verify: true }
    }
}

impl CargoPublish {
    /// Every option cargo has, for the error that names them.
    const OPTIONS: [&'static str; 1] = ["verify"];

    /// The options `package` wrote, over the defaults. The error names what
    /// to change, in the words of `.ctl/ver.yaml`.
    fn read(package: &str, written: Option<&PublishOptions>) -> Result<Self, String> {
        let mut options = Self::default();
        for (key, value) in written.into_iter().flatten() {
            match key.as_str() {
                "verify" => {
                    options.verify = value.as_bool().ok_or_else(|| {
                        format!("package {package:?}: publish.verify is true or false")
                    })?;
                }
                _ => {
                    return Err(format!(
                        "package {package:?}: cargo has no publish option {key:?}; its options are: {}",
                        Self::OPTIONS.join(", ")
                    ));
                }
            }
        }
        Ok(options)
    }

    /// The arguments these options add to the upload command.
    fn args(self) -> &'static [&'static str] {
        if self.verify { &[] } else { &["--no-verify"] }
    }

    /// What the plan shows beside the publisher's name: nothing while every
    /// option is at its default.
    fn note(self) -> Option<&'static str> {
        (!self.verify).then_some("no verify")
    }
}

/// How one package is published, resolved once from what `.ctl/ver.yaml`
/// declares and what the package's manifest states.
///
/// Every command that uploads, checks a registry, or asks whether a package
/// publishes reads this, so the answer is decided in one place.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Publisher {
    /// A tag and a Release, with no registry upload.
    ReleaseOnly,
    /// The built-in cargo upload. The crates.io index decides whether a
    /// version is published, and a virtual workspace uploads member by
    /// member.
    Cargo {
        /// The stock recipe under any `publishers.cargo` fields.
        recipe: PublisherSpec,
        /// The package's cargo options.
        options: CargoPublish,
    },
    /// The built-in bun upload, checked by its stock query.
    Bun {
        /// The stock recipe under any `publishers.bun` fields.
        recipe: PublisherSpec,
    },
    /// A command the repository wrote: a `publish` list on the package, or a
    /// `publishers` entry with its own `argv`. Only a `query` it declares
    /// checks it, because the registry it uploads to cannot be determined
    /// from the config.
    Command {
        /// The `publishers` key, or the package's own name.
        name: String,
        /// The recipe that holds the command.
        recipe: PublisherSpec,
    },
}

impl Publisher {
    /// The publisher of `spec` from `.ctl/ver.yaml` alone, reading no
    /// manifest. The error names what to change in the file.
    fn declared(
        config: &crate::config::Config,
        spec: &crate::config::PackageSpec,
    ) -> Result<Self, String> {
        let name = publisher_name(config, spec);
        if name == RELEASE_ONLY {
            return Ok(Self::ReleaseOnly);
        }
        let over = config.publishers.get(name);
        let stock = PublisherSpec::stock(name);
        let built_in = stock.is_some();
        let entry_replaces = over.is_some_and(|over| over.argv.is_some());
        let mut recipe = match (stock, over) {
            (Some(stock), Some(over)) => stock.merge(over.clone()),
            (Some(stock), None) => stock,
            (None, Some(over)) => over.clone(),
            (None, None) => {
                if spec.publish_command().is_none() {
                    return Err(format!(
                        "package {:?} has no publisher (set publisher / publishers.{name} / argv)",
                        spec.name
                    ));
                }
                PublisherSpec::default()
            }
        };
        if spec.noun.is_some() {
            recipe.noun.clone_from(&spec.noun);
        }
        if let Some(argv) = spec.publish_command() {
            recipe.argv = Some(argv.to_vec());
        }
        let written = spec.publish_options().filter(|options| !options.is_empty());
        if built_in && !entry_replaces && spec.publish_command().is_none() {
            if name == "cargo" {
                return Ok(Self::Cargo {
                    recipe,
                    options: CargoPublish::read(&spec.name, written)?,
                });
            }
            if let Some(options) = written {
                return Err(no_options(&spec.name, name, options));
            }
            return Ok(Self::Bun { recipe });
        }
        if let Some(options) = written {
            return Err(if built_in && entry_replaces {
                format!(
                    "package {:?}: publishers.{name} replaces the upload command, which takes no options; remove publish, or the argv",
                    spec.name
                )
            } else {
                no_options(&spec.name, name, options)
            });
        }
        Ok(Self::Command {
            name: name.to_owned(),
            recipe,
        })
    }

    /// The publisher of `spec` in the repository at `root`.
    ///
    /// A built-in publisher that was inferred from the manifest file name
    /// becomes [`Publisher::ReleaseOnly`] when that manifest opts out of
    /// publishing. A publisher the package names, and a command it writes,
    /// are used as written.
    ///
    /// # Errors
    ///
    /// Returns an error when the package has no publisher, or sets an option
    /// its publisher does not have. It also fails when the manifest cannot be
    /// read or parsed, or opts out while the package sets `registry` or
    /// `publish`.
    pub fn of(
        config: &crate::config::Config,
        spec: &crate::config::PackageSpec,
        root: &Path,
    ) -> Result<Self> {
        let (publisher, opted_out) = Self::settle(config, spec, root, root)?;
        if let Some(reason) = opted_out {
            ensure!(
                spec.registry.is_none() && spec.publish.is_none(),
                "package {:?} does not publish: {reason}; remove its publish and registry, or the manifest key",
                spec.name
            );
        }
        Ok(publisher)
    }

    /// Whether `spec` publishes, for a copy of the repository: `tree` is the
    /// root of the tree `root` is in, and no manifest lookup leaves it.
    ///
    /// It answers as [`Publisher::of`] does, apart from one case. A manifest
    /// that opts out while the package still sets `registry` or `publish`
    /// gives [`Publisher::ReleaseOnly`] here. The conflict fails the plan, and
    /// `check` reads a merge-base config that may hold it, such as on the
    /// pull request that removes the `registry`.
    pub(crate) fn of_in(
        config: &crate::config::Config,
        spec: &crate::config::PackageSpec,
        root: &Path,
        tree: &Path,
    ) -> Result<Self> {
        Ok(Self::settle(config, spec, root, tree)?.0)
    }

    /// The publisher of `spec`, and the manifest's reason when its opt-out
    /// turned an inferred built-in publisher into [`Publisher::ReleaseOnly`].
    fn settle(
        config: &crate::config::Config,
        spec: &crate::config::PackageSpec,
        root: &Path,
        tree: &Path,
    ) -> Result<(Self, Option<&'static str>)> {
        let declared = Self::declared(config, spec).map_err(anyhow::Error::msg)?;
        let built_in = matches!(declared, Self::Cargo { .. } | Self::Bun { .. });
        if spec.publisher.is_some() || !built_in {
            return Ok((declared, None));
        }
        Ok(
            match crate::opt_out::opt_out(&root.join(&spec.path), tree)? {
                Some(reason) => (Self::ReleaseOnly, Some(reason)),
                None => (declared, None),
            },
        )
    }
}

/// The error for options on a publisher that has none.
fn no_options(package: &str, publisher: &str, options: &PublishOptions) -> String {
    let keys: Vec<String> = options.keys().map(|key| format!("publish.{key}")).collect();
    format!(
        "package {package:?} publishes with {publisher}, which has no publish options; remove {}",
        keys.join(", ")
    )
}

/// Whether the `publish` options of `spec` belong to its publisher, checked
/// from `.ctl/ver.yaml` alone so the load can refuse them.
///
/// # Errors
///
/// Returns what to change when an option is unknown to the publisher, has the
/// wrong type, or sits on a publisher that takes none.
pub(crate) fn check_options(
    config: &crate::config::Config,
    spec: &crate::config::PackageSpec,
) -> Result<(), String> {
    if spec.publish_options().is_none() {
        return Ok(());
    }
    Publisher::declared(config, spec).map(|_| ())
}

/// The npm dist-tag of a candidate: the first identifier of the prerelease of
/// `version`, so `0.8.2-rc.1` gives `rc`. A stable version, and one that is not
/// semver, give none.
///
/// A registry that gets a prerelease with no tag files it under `latest`, which
/// would hand the candidate to every consumer that names no version.
pub(crate) fn prerelease_tag(version: &str) -> Option<String> {
    let version = semver::Version::parse(version).ok()?;
    version
        .pre
        .as_str()
        .split('.')
        .next()
        .filter(|tag| !tag.is_empty())
        .map(str::to_owned)
}

/// Resolve a package's label, noun, and upload command.
///
/// The label is the plan's `via`: the publisher, then the registry when one
/// is named, then any option that differs from its default.
///
/// This is the command for a stable version. [`resolve_for_version`] adds what
/// a candidate needs.
///
/// # Errors
///
/// Returns an error when [`Publisher::of`] fails, when the publisher declares
/// no `argv`, or when the rendered command is empty, which only
/// `publisher: none` may be.
pub fn resolve(
    config: &crate::config::Config,
    spec: &crate::config::PackageSpec,
    root: &Path,
) -> Result<(String, String, Vec<String>)> {
    resolve_tagged(config, spec, root, None)
}

/// [`resolve`] for the package at `version`.
///
/// The stock bun upload of a candidate adds `--tag <prerelease tag>`. cargo
/// and a command the repository wrote upload as they always do.
///
/// # Errors
///
/// Returns the errors of [`resolve`].
pub fn resolve_for_version(
    config: &crate::config::Config,
    spec: &crate::config::PackageSpec,
    root: &Path,
    version: &str,
) -> Result<(String, String, Vec<String>)> {
    resolve_tagged(config, spec, root, prerelease_tag(version).as_deref())
}

/// [`resolve`], with the npm dist-tag the stock bun upload carries, if any.
fn resolve_tagged(
    config: &crate::config::Config,
    spec: &crate::config::PackageSpec,
    root: &Path,
    dist_tag: Option<&str>,
) -> Result<(String, String, Vec<String>)> {
    let publisher = Publisher::of(config, spec, root)?;
    let (name, recipe, options) = match &publisher {
        Publisher::ReleaseOnly => {
            let noun = spec.noun.clone().unwrap_or_else(|| "package".into());
            return Ok(("release only".into(), noun, Vec::new()));
        }
        Publisher::Cargo { recipe, options } => ("cargo", recipe, Some(*options)),
        Publisher::Bun { recipe } => ("bun", recipe, None),
        Publisher::Command { name, recipe } => (name.as_str(), recipe, None),
    };
    let manifest = root.join(&spec.path);
    let (noun, mut argv) = recipe.render(name, &manifest, spec.registry.as_deref())?;
    ensure!(
        !argv.is_empty(),
        "package {:?}: publisher {name:?} renders an empty command; set publisher: none to release without a registry",
        spec.name
    );
    let mut label = name.to_owned();
    if let Some(registry) = spec
        .registry
        .as_deref()
        .filter(|registry| !registry.is_empty())
    {
        label = format!("{label} {registry}");
    }
    if let Some(options) = options {
        argv.extend(options.args().iter().map(|part| (*part).to_owned()));
        if let Some(note) = options.note() {
            label = format!("{label}, {note}");
        }
    }
    if matches!(publisher, Publisher::Bun { .. })
        && let Some(tag) = dist_tag
    {
        argv.extend(["--tag".to_owned(), tag.to_owned()]);
    }
    Ok((label, noun, argv))
}

/// The publisher a package names, tried in order.
///
/// 1. The `publisher` it declares.
/// 2. The stock one its manifest file implies, unless the version is on a
///    marker line: a marker in a `Cargo.toml` says nothing about crates.io.
/// 3. A `publishers` entry or `publish` command under the package's own name.
/// 4. `none` for a version source no registry reads, such as `go.mod`,
///    `VERSION`, or a marker line, whose release is its tag.
pub(crate) fn publisher_name<'a>(
    config: &crate::config::Config,
    spec: &'a crate::config::PackageSpec,
) -> &'a str {
    if let Some(name) = spec.publisher.as_deref() {
        return name;
    }
    let file_name = spec.path.file_name().and_then(|name| name.to_str());
    if !spec.marker
        && let Some(stock) = file_name.and_then(stock_publisher_for_file)
    {
        return stock;
    }
    let names_its_own =
        spec.publish_command().is_some() || config.publishers.contains_key(&spec.name);
    if !names_its_own && (spec.marker || file_name.is_some_and(releases_on_github_only)) {
        return RELEASE_ONLY;
    }
    spec.name.as_str()
}

/// Version sources whose release is a tag and a Release. A Go module is
/// fetched from its tag, and a `VERSION` file names no registry at all, so
/// no upload exists to run.
fn releases_on_github_only(file_name: &str) -> bool {
    matches!(file_name, "go.mod" | "VERSION")
}

/// How `release` decides whether `version` of a package is already on its
/// registry.
///
/// A package's own `query` wins, then its `publishers` entry's, and an empty
/// one turns the check off. Otherwise [`Publisher::Cargo`] bound for crates.io
/// is read from the sparse index, [`Publisher::Bun`] runs its stock query,
/// and [`Publisher::Command`] has no check.
///
/// The registry is asked for the name the manifest declares (`package.name`
/// in `Cargo.toml`, `name` in `package.json`). The label `.ctl/ver.yaml` gives
/// the package is unused.
///
/// A stock query takes the recipe's registry or config arguments, so it
/// targets the registry the upload goes to. A declared query runs as written.
/// It uses `{registry}` and `{config}` to place those arguments itself.
///
/// # Errors
///
/// Returns an error when [`Publisher::of`] fails, or when the manifest cannot
/// be read or parsed, or names no package.
pub fn registry_check(
    config: &crate::config::Config,
    spec: &crate::config::PackageSpec,
    root: &Path,
    version: &str,
) -> Result<RegistryCheck> {
    let publisher = Publisher::of(config, spec, root)?;
    let (name, recipe) = match &publisher {
        Publisher::ReleaseOnly => return Ok(RegistryCheck::Unchecked),
        Publisher::Cargo { recipe, .. } => ("cargo", recipe),
        Publisher::Bun { recipe } => ("bun", recipe),
        Publisher::Command { name, recipe } => (name.as_str(), recipe),
    };
    let manifest = root.join(&spec.path);
    let declared = spec.query.as_ref().or_else(|| {
        config
            .publishers
            .get(name)
            .and_then(|over| over.query.as_ref())
    });
    if let Some(argv) = declared {
        return query_check(recipe, argv, spec, &manifest, version, Appended::Nothing);
    }
    match &publisher {
        Publisher::ReleaseOnly | Publisher::Command { .. } => Ok(RegistryCheck::Unchecked),
        Publisher::Cargo { .. } => {
            let target = registry::cargo_target(
                spec.registry.as_deref(),
                &manifest,
                root,
                &CargoEnv::from_env(),
            )?;
            Ok(match target {
                CargoTarget::CratesIo => RegistryCheck::CratesIo {
                    name: manifest_name(&manifest)?,
                },
                CargoTarget::Other | CargoTarget::Unknown => RegistryCheck::Unchecked,
            })
        }
        Publisher::Bun { recipe } => match &recipe.query {
            Some(argv) => query_check(recipe, argv, spec, &manifest, version, Appended::StockQuery),
            None => Ok(RegistryCheck::Unchecked),
        },
    }
}

/// One upload `release` runs: a package, or one member of a workspace package.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Upload {
    /// The package name, or the member's crate name.
    pub name: String,
    /// The version it uploads.
    pub version: String,
    /// Its manifest.
    pub manifest: PathBuf,
    /// The upload command.
    pub argv: Vec<String>,
    /// How `release` asks whether the version is already published.
    pub check: RegistryCheck,
}

/// The uploads that publish `entry`, in the order they run.
///
/// A workspace package (see [`workspace_members`]) runs one
/// `cargo publish --package <member>` per member that publishes, in
/// dependency order. Each carries the package's cargo options and is checked
/// against its own target. Any other package runs its one upload command
/// under [`registry_check`].
///
/// # Errors
///
/// Returns an error when [`workspace_members`] or [`registry_check`] fails.
pub fn uploads(
    config: &crate::config::Config,
    spec: &crate::config::PackageSpec,
    root: &Path,
    entry: &crate::publish::PublishEntry,
) -> Result<Vec<Upload>> {
    let manifest = root.join(&spec.path);
    let Some((recipe, options, members)) =
        workspace(&Publisher::of(config, spec, root)?, config, spec, root)?
    else {
        return Ok(vec![Upload {
            name: entry.name.clone(),
            version: entry.version.clone(),
            manifest,
            check: registry_check(config, spec, root, &entry.version)?,
            argv: entry.argv.clone(),
        }]);
    };
    let (_, mut base) = recipe.render("cargo", &manifest, spec.registry.as_deref())?;
    base.extend(options.args().iter().map(|part| (*part).to_owned()));
    let env = CargoEnv::from_env();
    Ok(members
        .into_iter()
        .map(|member| {
            let mut argv = base.clone();
            argv.extend(["--package".to_owned(), member.name.clone()]);
            let target = registry::cargo_member_target(
                spec.registry.as_deref(),
                member.publish.as_deref(),
                root,
                &env,
            );
            let check = match target {
                CargoTarget::CratesIo => RegistryCheck::CratesIo {
                    name: member.name.clone(),
                },
                CargoTarget::Other | CargoTarget::Unknown => RegistryCheck::Unchecked,
            };
            Upload {
                name: member.name,
                version: member.version,
                manifest: member.manifest,
                argv,
                check,
            }
        })
        .collect())
}

/// The members a workspace package publishes, in publish order, or `None`
/// when the package is not one.
///
/// A workspace package:
///
/// - resolves to [`Publisher::Cargo`], so nothing replaces its upload command;
/// - declares no `query`, on the package or on `publishers.cargo`;
/// - points `path` at a virtual workspace manifest, whose root names no crate
///   to publish.
///
/// Declaring `publish` as a list on the package runs that one command
/// instead. `publish` options keep the package a workspace.
///
/// # Errors
///
/// Returns an error when [`Publisher::of`] fails, when the manifest cannot be
/// read or parsed, or when [`members::members`] fails.
pub fn workspace_members(
    config: &crate::config::Config,
    spec: &crate::config::PackageSpec,
    root: &Path,
) -> Result<Option<Vec<members::Member>>> {
    Publisher::of(config, spec, root)?.members(config, spec, root)
}

impl Publisher {
    /// The members this publisher uploads one by one, in publish order, or
    /// `None` when it uploads the package as one; see [`workspace_members`].
    ///
    /// # Errors
    ///
    /// Returns an error when the manifest cannot be read or parsed, or when
    /// [`members::members`] fails.
    pub fn members(
        &self,
        config: &crate::config::Config,
        spec: &crate::config::PackageSpec,
        root: &Path,
    ) -> Result<Option<Vec<members::Member>>> {
        Ok(workspace(self, config, spec, root)?.map(|(_, _, members)| members))
    }
}

/// A workspace package's cargo recipe, its options, and its members; see
/// [`workspace_members`].
fn workspace(
    publisher: &Publisher,
    config: &crate::config::Config,
    spec: &crate::config::PackageSpec,
    root: &Path,
) -> Result<Option<(PublisherSpec, CargoPublish, Vec<members::Member>)>> {
    let Publisher::Cargo { recipe, options } = publisher else {
        return Ok(None);
    };
    let declares_query = spec.query.is_some()
        || config
            .publishers
            .get("cargo")
            .is_some_and(|over| over.query.is_some());
    let manifest = root.join(&spec.path);
    if declares_query || !members::is_virtual(&manifest)? {
        return Ok(None);
    }
    Ok(Some((
        recipe.clone(),
        *options,
        members::members(&manifest)?,
    )))
}

/// A query check whose `{name}` is the name the manifest declares, read only
/// when the query uses it. A `Cargo.toml` or `package.json` must name its
/// package, since asking for the label instead could find another package;
/// any other manifest (`pyproject.toml`, a directory) uses the label.
fn query_check(
    publisher: &PublisherSpec,
    argv: &[String],
    spec: &crate::config::PackageSpec,
    manifest: &Path,
    version: &str,
    appended: Appended,
) -> Result<RegistryCheck> {
    if argv.is_empty() {
        return Ok(RegistryCheck::Unchecked);
    }
    let readable = matches!(
        manifest.file_name().and_then(|name| name.to_str()),
        Some("Cargo.toml" | "package.json")
    );
    let name = if readable && argv.iter().any(|part| part.contains("{name}")) {
        manifest_name(manifest)?
    } else {
        spec.name.clone()
    };
    let extra = [("name", name), ("version", version.to_owned())];
    Ok(RegistryCheck::Query {
        argv: publisher.assemble(argv, manifest, spec.registry.as_deref(), &extra, appended)?,
        dir: package_dir(manifest).to_path_buf(),
    })
}

/// The package name a manifest declares: `package.name` in `Cargo.toml`,
/// `name` in `package.json`, which is the name the registry lists it under.
fn manifest_name(manifest: &Path) -> Result<String> {
    let raw = std::fs::read_to_string(manifest).with_context(|| manifest.display().to_string())?;
    let name = if manifest
        .file_name()
        .is_some_and(|name| name == "package.json")
    {
        serde_json::from_str::<serde_json::Value>(&raw)
            .with_context(|| manifest.display().to_string())?
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    } else {
        raw.parse::<toml_edit::DocumentMut>()
            .with_context(|| manifest.display().to_string())?
            .get("package")
            .and_then(|package| package.get("name"))
            .and_then(toml_edit::Item::as_str)
            .map(str::to_owned)
    };
    name.with_context(|| format!("{} names no package", manifest.display()))
}

fn stock_publisher_for_file(file_name: &str) -> Option<&'static str> {
    match file_name {
        "Cargo.toml" => Some("cargo"),
        "package.json" => Some("bun"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;

    use indoc::{formatdoc, indoc};

    use super::{PublisherSpec, expand, registry_check, resolve, resolve_for_version};
    use crate::registry::RegistryCheck;

    fn words(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_owned()).collect()
    }

    #[test]
    fn a_misspelt_placeholder_fails_instead_of_dropping_its_argument() {
        let ctx = BTreeMap::from([("dir", "pkg".to_owned())]);
        let error = expand(&words(&["publish", "--registry", "{registr}"]), &ctx)
            .unwrap_err()
            .to_string();
        assert!(error.contains("{registr}"), "{error}");
        assert!(error.contains("{dir}"), "{error}");
    }

    #[test]
    fn a_known_placeholder_without_a_value_fails() {
        let ctx = BTreeMap::from([("dir", "pkg".to_owned())]);
        let error = expand(&words(&["publish", "--registry", "{registry}"]), &ctx)
            .unwrap_err()
            .to_string();
        assert!(error.contains("{registry}"), "{error}");
    }

    #[test]
    fn braces_that_are_not_a_placeholder_pass_through() {
        let ctx = BTreeMap::from([("dir", "pkg".to_owned())]);
        let argv = expand(&words(&["--meta", r#"{"a": 1}"#, "{dir}"]), &ctx).unwrap();
        assert_eq!(argv, words(&["--meta", r#"{"a": 1}"#, "pkg"]));
    }

    #[test]
    fn stock_cargo_and_bun_are_recipes() {
        let cargo = PublisherSpec::stock("cargo").unwrap();
        let (_, argv) = cargo
            .render("cargo", Path::new("Cargo.toml"), None)
            .unwrap();
        assert_eq!(
            argv,
            [
                "cargo",
                "publish",
                "--locked",
                "--manifest-path",
                "Cargo.toml"
            ]
        );
        let bun = PublisherSpec::stock("bun").unwrap();
        let (_, argv) = bun
            .render("bun", Path::new("packages/pkg/package.json"), None)
            .unwrap();
        assert_eq!(
            argv,
            [
                "bun",
                "publish",
                "--tolerate-republish",
                "--cwd",
                "packages/pkg",
                "--access",
                "public",
            ]
        );
    }

    /// Resolves the upload of the first package of `yaml` at `version`, in
    /// the tree `manifests` builds, with the root a placeholder renders into.
    fn upload_at(yaml: &str, version: &str) -> (Vec<String>, String) {
        let root = manifests();
        let config = crate::config::Config::parse(yaml).unwrap();
        let (_, _, argv) =
            resolve_for_version(&config, &config.packages[0], root.path(), version).unwrap();
        (argv, root.path().display().to_string())
    }

    #[test]
    fn a_bun_candidate_uploads_under_its_prerelease_tag() {
        let (argv, root) = upload_at(
            indoc! {"
                packages:
                  - name: web
                    path: web/package.json
            "},
            "1.3.0-rc.1",
        );
        assert_eq!(
            argv,
            [
                "bun",
                "publish",
                "--tolerate-republish",
                "--cwd",
                &format!("{root}/web"),
                "--access",
                "public",
                "--tag",
                "rc",
            ]
        );
    }

    #[test]
    fn a_cargo_candidate_uploads_with_its_usual_argv() {
        let (argv, root) = upload_at(
            indoc! {"
                packages:
                  - name: demo
                    path: demo/Cargo.toml
            "},
            "0.8.0-rc.1",
        );
        assert_eq!(
            argv,
            [
                "cargo",
                "publish",
                "--locked",
                "--manifest-path",
                &format!("{root}/demo/Cargo.toml"),
            ]
        );
    }

    #[test]
    fn a_command_candidate_uploads_with_its_usual_argv() {
        let (argv, _) = upload_at(
            indoc! {"
                packages:
                  - name: demo
                    path: demo/Cargo.toml
                    publish: [goreleaser, release]
            "},
            "0.8.0-rc.1",
        );
        assert_eq!(argv, ["goreleaser", "release"]);
    }

    #[test]
    fn a_stable_bun_version_has_no_tag() {
        let (argv, _) = upload_at(
            indoc! {"
                packages:
                  - name: web
                    path: web/package.json
            "},
            "1.3.0",
        );
        assert!(!argv.iter().any(|part| part == "--tag"), "{argv:?}");
    }

    #[test]
    fn the_tag_is_the_first_prerelease_identifier() {
        assert_eq!(super::prerelease_tag("0.8.2-rc.1").as_deref(), Some("rc"));
        assert_eq!(super::prerelease_tag("1.0.0-beta").as_deref(), Some("beta"));
        assert_eq!(super::prerelease_tag("0.8.2"), None);
        assert_eq!(super::prerelease_tag("not a version"), None);
    }

    /// An empty upload command means release-only, so only `publisher: none`
    /// may produce one.
    #[test]
    fn only_none_renders_an_empty_command() {
        let config = crate::config::Config::parse(indoc! {"
            packages:
              - name: quiet
                path: Cargo.toml
                publisher: none
              - name: empty
                path: Cargo.toml
                publish: []
        "})
        .unwrap();
        let root = Path::new("");
        let (label, _, argv) = resolve(&config, &config.packages[0], root).unwrap();
        assert_eq!(label, "release only");
        assert_eq!(argv, Vec::<String>::new());
        let error = resolve(&config, &config.packages[1], root).unwrap_err();
        assert!(
            format!("{error:#}").contains("renders an empty command"),
            "{error:#}"
        );
    }

    /// A Go module is fetched from its tag. A `VERSION` file or a marker line
    /// names no registry. With no publisher declared, each releases on GitHub
    /// only. A publisher the package names still wins.
    #[test]
    fn version_sources_with_no_registry_release_on_github_only() {
        let config = crate::config::Config::parse(indoc! {"
            packages:
              - name: lib
                path: go.mod
              - name: tool
                path: tools/go.mod
                publish: [goreleaser, release]
              - name: templates
                path: VERSION
              - name: ci
                path: ci.yml
                marker: true
              - name: sample
                path: sample/Cargo.toml
                marker: true
        "})
        .unwrap();
        let (label, _, _) = resolve(&config, &config.packages[3], Path::new("")).unwrap();
        assert_eq!(label, "release only");
        let (label, _, _) = resolve(&config, &config.packages[4], Path::new("")).unwrap();
        assert_eq!(label, "release only");
        let (label, _, argv) = resolve(&config, &config.packages[0], Path::new("")).unwrap();
        assert_eq!((label.as_str(), argv.len()), ("release only", 0));
        let (label, _, argv) = resolve(&config, &config.packages[2], Path::new("")).unwrap();
        assert_eq!((label.as_str(), argv.len()), ("release only", 0));
        let (_, _, argv) = resolve(&config, &config.packages[1], Path::new("")).unwrap();
        assert_eq!(argv, ["goreleaser", "release"]);
        assert_eq!(
            registry_check(&config, &config.packages[0], Path::new("."), "0.1.0").unwrap(),
            RegistryCheck::Unchecked
        );
        let error = crate::config::Config::parse(indoc! {"
            packages:
              - name: lib
                path: go.mod
                registry: proxy
        "})
        .expect_err("a registry nothing reads");
        assert!(
            format!("{error:#}").contains("publisher: none"),
            "{error:#}"
        );
    }

    fn checks(yaml: &str) -> Vec<RegistryCheck> {
        let root = tempfile::TempDir::new().unwrap();
        std::fs::write(
            root.path().join("Cargo.toml"),
            indoc! {r#"
                [package]
                name = "demo"
                version = "0.4.6"
            "#},
        )
        .unwrap();
        std::fs::write(root.path().join("package.json"), r#"{"name": "@acme/web"}"#).unwrap();
        let config = crate::config::Config::parse(yaml).unwrap();
        config
            .packages
            .iter()
            .map(|spec| registry_check(&config, spec, root.path(), "0.4.6").unwrap())
            .collect()
    }

    #[test]
    fn stock_cargo_bound_for_crates_io_reads_the_index() {
        let found = checks(indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
        "});
        assert_eq!(
            found,
            [RegistryCheck::CratesIo {
                name: "demo".into()
            }]
        );
    }

    #[test]
    fn an_overridden_cargo_publisher_has_no_check() {
        let found = checks(indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
                publish:
                  - cargo
                  - publish
              - name: elsewhere
                path: Cargo.toml
                registry: my-registry
              - name: quiet
                path: Cargo.toml
                publisher: none
        "});
        assert_eq!(found, vec![RegistryCheck::Unchecked; 3]);
    }

    /// A bun package's own upload command may name another registry, so the
    /// stock `bun pm view` query would ask the wrong one.
    #[test]
    fn an_overridden_bun_publisher_has_no_check() {
        let found = checks(indoc! {"
            packages:
              - name: web
                path: package.json
                publish:
                  - npm
                  - publish
                  - --registry
                  - https://registry.example.com
        "});
        assert_eq!(found, vec![RegistryCheck::Unchecked]);
    }

    #[test]
    fn a_package_query_wins_and_an_empty_one_turns_the_check_off() {
        let found = checks(indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
                query:
                  - registry-cli
                  - '{name}@{version}'
              - name: off
                path: Cargo.toml
                query: []
        "});
        let RegistryCheck::Query { argv, .. } = &found[0] else {
            panic!("expected a query, got {:?}", found[0]);
        };
        assert_eq!(argv, &["registry-cli", "demo@0.4.6"]);
        assert_eq!(found[1], RegistryCheck::Unchecked);
    }

    #[test]
    fn stock_bun_queries_the_registry() {
        let found = checks(indoc! {"
            packages:
              - name: web
                path: package.json
                publisher: bun
        "});
        let RegistryCheck::Query { argv, .. } = &found[0] else {
            panic!("expected a query, got {:?}", found[0]);
        };
        assert_eq!(argv, &["bun", "pm", "view", "@acme/web@0.4.6", "version"]);
    }

    #[test]
    fn a_bun_query_asks_the_registry_the_upload_goes_to() {
        let found = checks(indoc! {"
            packages:
              - name: web
                path: package.json
                publisher: bun
                registry: github
        "});
        let RegistryCheck::Query { argv, .. } = &found[0] else {
            panic!("expected a query, got {:?}", found[0]);
        };
        assert_eq!(
            argv,
            &[
                "bun",
                "pm",
                "view",
                "@acme/web@0.4.6",
                "version",
                "--registry",
                "https://npm.pkg.github.com",
            ]
        );
    }

    #[test]
    fn a_declared_query_runs_as_written() {
        let found = checks(indoc! {"
            packages:
              - name: web
                path: package.json
                publisher: bun
                registry: github
                query:
                  - registry-cli
                  - view
                  - '{name}'
                  - --from
                  - '{registry}'
        "});
        let RegistryCheck::Query { argv, .. } = &found[0] else {
            panic!("expected a query, got {:?}", found[0]);
        };
        assert_eq!(
            argv,
            &[
                "registry-cli",
                "view",
                "@acme/web",
                "--from",
                "https://npm.pkg.github.com"
            ]
        );
    }

    #[test]
    fn the_index_is_asked_for_the_manifest_name_not_the_label() {
        let found = checks(indoc! {"
            packages:
              - name: label
                path: Cargo.toml
        "});
        assert_eq!(
            found,
            [RegistryCheck::CratesIo {
                name: "demo".into()
            }]
        );
    }

    /// A virtual workspace with one library member and one tool crate.
    fn workspace() -> tempfile::TempDir {
        let root = tempfile::TempDir::new().unwrap();
        let write = |path: &str, body: &str| {
            let path = root.path().join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        };
        write(
            "Cargo.toml",
            indoc! {r#"
                [workspace]
                resolver = "3"
                members = ["core", "xtask"]

                [workspace.package]
                version = "0.0.2"
                edition = "2024"
            "#},
        );
        write(
            "core/Cargo.toml",
            indoc! {r#"
                [package]
                name = "demo-core"
                version.workspace = true
                edition.workspace = true
            "#},
        );
        write("core/src/lib.rs", "");
        write(
            "xtask/Cargo.toml",
            indoc! {r#"
                [package]
                name = "xtask"
                version = "0.1.0"
                edition.workspace = true
                publish = false
            "#},
        );
        write("xtask/src/main.rs", "fn main() {}");
        root
    }

    fn uploads_for(yaml: &str, root: &Path) -> Vec<super::Upload> {
        let config = crate::config::Config::parse(yaml).unwrap();
        let spec = &config.packages[0];
        let (label, noun, argv) = resolve(&config, spec, root).unwrap();
        let entry = crate::publish::PublishEntry {
            name: spec.name.clone(),
            version: "0.0.2".into(),
            noun,
            label,
            argv,
        };
        super::uploads(&config, spec, root, &entry).unwrap()
    }

    #[test]
    fn a_virtual_workspace_uploads_each_member_against_the_index() {
        let root = workspace();
        let found = uploads_for(
            indoc! {"
                packages:
                  - name: demo
                    path: Cargo.toml
            "},
            root.path(),
        );
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].name, "demo-core");
        assert_eq!(found[0].version, "0.0.2");
        assert_eq!(
            found[0].check,
            RegistryCheck::CratesIo {
                name: "demo-core".into()
            }
        );
        assert_eq!(
            found[0].argv[found[0].argv.len() - 2..],
            ["--package", "demo-core"]
        );
    }

    #[test]
    fn an_overridden_workspace_upload_stays_one_command() {
        let root = workspace();
        let found = uploads_for(
            indoc! {"
                packages:
                  - name: demo
                    path: Cargo.toml
                    publish:
                      - cargo
                      - publish
                      - --workspace
            "},
            root.path(),
        );
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].name, "demo");
        assert_eq!(found[0].argv, ["cargo", "publish", "--workspace"]);
        assert_eq!(found[0].check, RegistryCheck::Unchecked);
    }

    #[test]
    fn custom_argv_is_the_whole_recipe() {
        let spec = PublisherSpec {
            noun: Some("wheel".into()),
            argv: Some(vec!["uv".into(), "publish".into()]),
            ..PublisherSpec::default()
        };
        let (noun, argv) = spec
            .render("uv", Path::new("pyproject.toml"), None)
            .unwrap();
        assert_eq!(noun, "wheel");
        assert_eq!(argv, ["uv", "publish"]);
    }

    fn cargo_options(json: &str) -> Result<super::CargoPublish, String> {
        let written: crate::config::PublishOptions = serde_json::from_str(json).unwrap();
        super::CargoPublish::read("demo", Some(&written))
    }

    #[test]
    fn cargo_verifies_unless_told_not_to() {
        assert!(super::CargoPublish::read("demo", None).unwrap().verify);
        assert!(cargo_options(r#"{"verify": true}"#).unwrap().verify);
        assert!(!cargo_options(r#"{"verify": false}"#).unwrap().verify);
        assert_eq!(
            cargo_options(r#"{"verify": "no"}"#).unwrap_err(),
            r#"package "demo": publish.verify is true or false"#
        );
        assert_eq!(
            cargo_options(r#"{"allow_dirty": true}"#).unwrap_err(),
            r#"package "demo": cargo has no publish option "allow_dirty"; its options are: verify"#
        );
    }

    /// One line per publisher, so a table of packages reads as one list.
    fn shape(publisher: &super::Publisher) -> String {
        match publisher {
            super::Publisher::ReleaseOnly => "release only".into(),
            super::Publisher::Cargo { options, .. } if options.verify => "cargo".into(),
            super::Publisher::Cargo { .. } => "cargo, no verify".into(),
            super::Publisher::Bun { .. } => "bun".into(),
            super::Publisher::Command { name, .. } => format!("command {name}"),
        }
    }

    /// A tree whose manifests cover each way a package can publish.
    fn manifests() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let write = |path: &str, text: &str| {
            let path = root.path().join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        let crate_toml = |name: &str, publish: &str| {
            formatdoc! {r#"
                [package]
                name = "{name}"
                version = "0.1.0"
                {publish}
            "#}
        };
        for name in ["quiet", "demo", "listed"] {
            write(&format!("{name}/Cargo.toml"), &crate_toml(name, ""));
        }
        write("named/Cargo.toml", &crate_toml("named", "publish = false"));
        write(
            "private/Cargo.toml",
            &crate_toml("private", "publish = false"),
        );
        write("closed/Cargo.toml", &crate_toml("closed", "publish = []"));
        write("web/package.json", r#"{"name": "web"}"#);
        write(
            "secret/package.json",
            r#"{"name": "secret", "private": true}"#,
        );
        write(
            "kin/Cargo.toml",
            indoc! {r#"
                [workspace]
                members = ["member"]

                [workspace.package]
                publish = false
            "#},
        );
        write(
            "kin/member/Cargo.toml",
            indoc! {r#"
                [package]
                name = "member"
                version = "0.1.0"
                publish.workspace = true
            "#},
        );
        root
    }

    #[test]
    fn each_resolution_step_picks_one_publisher() {
        let root = manifests();
        let config = crate::config::Config::parse(indoc! {"
            packages:
              - name: quiet
                path: quiet/Cargo.toml
                publisher: none
              - name: named
                path: named/Cargo.toml
                publisher: cargo
              - name: demo
                path: demo/Cargo.toml
                publish:
                  verify: false
              - name: web
                path: web/package.json
              - name: lib
                path: go.mod
              - name: listed
                path: listed/Cargo.toml
                publish: [cargo, publish, --workspace]
              - name: private
                path: private/Cargo.toml
              - name: closed
                path: closed/Cargo.toml
              - name: member
                path: kin/member/Cargo.toml
              - name: secret
                path: secret/package.json
        "})
        .unwrap();
        let found: Vec<String> = config
            .packages
            .iter()
            .map(|spec| shape(&super::Publisher::of(&config, spec, root.path()).unwrap()))
            .collect();
        assert_eq!(
            found,
            [
                "release only",
                "cargo",
                "cargo, no verify",
                "bun",
                "release only",
                "command cargo",
                "release only",
                "release only",
                "release only",
                "release only",
            ]
        );
    }

    #[test]
    fn a_publishers_entry_with_its_own_argv_is_a_command() {
        let root = manifests();
        let config = crate::config::Config::parse(indoc! {"
            publishers:
              bun:
                argv: [npm, publish]
            packages:
              - name: web
                path: web/package.json
              - name: secret
                path: secret/package.json
        "})
        .unwrap();
        let found: Vec<String> = config
            .packages
            .iter()
            .map(|spec| shape(&super::Publisher::of(&config, spec, root.path()).unwrap()))
            .collect();
        assert_eq!(found, ["command bun", "command bun"]);
    }

    /// The plan refuses the conflict. `check`, which may read it from a
    /// merge-base config, still finds that the package does not publish.
    #[test]
    fn options_beside_a_manifest_that_opts_out_fail_the_plan_only() {
        let root = manifests();
        let config = crate::config::Config::parse(indoc! {"
            packages:
              - name: private
                path: private/Cargo.toml
                publish:
                  verify: false
        "})
        .unwrap();
        let error = super::Publisher::of(&config, &config.packages[0], root.path()).unwrap_err();
        assert_eq!(
            error.to_string(),
            r#"package "private" does not publish: Cargo.toml sets publish = false; remove its publish and registry, or the manifest key"#
        );
        let check = super::Publisher::of_in(&config, &config.packages[0], root.path(), root.path())
            .unwrap();
        assert_eq!(shape(&check), "release only");
    }

    #[test]
    fn a_crate_ci_already_packed_skips_the_verify_build() {
        let root = manifests();
        let config = crate::config::Config::parse(indoc! {"
            packages:
              - name: demo
                path: demo/Cargo.toml
                publish:
                  verify: false
              - name: listed
                path: listed/Cargo.toml
                registry: internal
                publish:
                  verify: false
              - name: quiet
                path: quiet/Cargo.toml
        "})
        .unwrap();
        let (label, _, argv) = resolve(&config, &config.packages[0], root.path()).unwrap();
        assert_eq!(label, "cargo, no verify");
        assert_eq!(argv.last().map(String::as_str), Some("--no-verify"));
        let (label, _, _) = resolve(&config, &config.packages[1], root.path()).unwrap();
        assert_eq!(label, "cargo internal, no verify");
        let (label, _, argv) = resolve(&config, &config.packages[2], root.path()).unwrap();
        assert_eq!(label, "cargo");
        assert!(!argv.iter().any(|part| part == "--no-verify"), "{argv:?}");
    }

    #[test]
    fn options_keep_the_registry_check() {
        let found = checks(indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
                publish:
                  verify: false
        "});
        assert_eq!(
            found,
            [RegistryCheck::CratesIo {
                name: "demo".into()
            }]
        );
    }

    #[test]
    fn options_reach_every_member() {
        let root = workspace();
        let found = uploads_for(
            indoc! {"
                packages:
                  - name: demo
                    path: Cargo.toml
                    publish:
                      verify: false
            "},
            root.path(),
        );
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(
            found[0].check,
            RegistryCheck::CratesIo {
                name: "demo-core".into()
            }
        );
        assert_eq!(
            found[0].argv[found[0].argv.len() - 3..],
            ["--no-verify", "--package", "demo-core"]
        );
    }
}
