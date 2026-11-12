use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use ctl_core::input::{FieldPath, Input};
use ctl_core::validate::{at_least_one, cannot_be_empty, inside_the_repo};
use garde::Validate;
use serde::Deserialize;

use crate::driver::{CommandSpec, Driver, Format, Text};
use crate::publisher::PublisherSpec;

/// A driver's read or write step: a mise task or an argv.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum CommandField {
    /// A `mise run` task name.
    Mise(String),
    /// A program and its arguments, run without a shell.
    Argv(Vec<String>),
}

impl CommandField {
    fn into_spec(self) -> Result<CommandSpec> {
        match self {
            Self::Mise(task) => {
                if task.is_empty() {
                    bail!("mise task name is empty");
                }
                Ok(CommandSpec::Mise(task))
            }
            Self::Argv(argv) => {
                ensure!(!argv.is_empty(), "driver argv is empty");
                Ok(CommandSpec::Argv(argv))
            }
        }
    }
}

/// A version driver as declared, before it becomes a [`Driver`]. `format` and
/// `keys` edit the manifest in place. `read` and `write` run commands instead
/// and take precedence.
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct DriverSpec {
    /// `toml` or `json`, edited at `keys`. `gomod` (the `// vX.Y.Z` comment on
    /// a `go.mod` module line) and `version` (a file holding only the
    /// version) take no keys.
    pub format: Option<String>,
    /// Key paths holding the version, tried in order.
    pub keys: Option<Vec<String>>,
    /// Command that takes the manifest on stdin and prints the version.
    pub read: Option<CommandField>,
    /// Command that takes the manifest on stdin and prints it with the new
    /// version.
    pub write: Option<CommandField>,
    /// Command run after the bumps, such as a lockfile refresh. Inferred from
    /// the tree for stock manifests when omitted.
    pub after: Option<String>,
}

impl DriverSpec {
    /// Build the [`Driver`]. `name` only labels the errors.
    ///
    /// # Errors
    ///
    /// Returns an error when a `read` and `write` pair has an empty mise task
    /// name or an empty argv. Without that pair, it returns an error when
    /// `format` is missing or not `toml`, `json`, `gomod`, or `version`, when
    /// `keys` is missing or empty for `toml` or `json`, or when `gomod` or
    /// `version` has `keys`.
    pub fn into_driver(self, name: &str) -> Result<Driver> {
        if let (Some(read), Some(write)) = (self.read, self.write) {
            return Ok(Driver::Command {
                read: read.into_spec()?,
                write: write.into_spec()?,
                after: self.after,
            });
        }
        let format = match self.format.as_deref() {
            Some("toml") => Format::Toml,
            Some("json") => Format::Json,
            Some(text @ ("gomod" | "version")) => {
                ensure!(
                    self.keys.is_none(),
                    "driver {name:?} has format {text}, which reads the whole file and takes no keys"
                );
                let text = if text == "gomod" {
                    Text::GoMod
                } else {
                    Text::Version
                };
                return Ok(Driver::Text {
                    text,
                    after: self.after,
                });
            }
            Some(other) => bail!("driver {name:?} has unknown format {other:?}"),
            None => bail!("driver {name:?} needs format+keys or read+write"),
        };
        let keys = self
            .keys
            .filter(|keys| !keys.is_empty())
            .with_context(|| format!("driver {name:?} needs keys"))?;
        Ok(Driver::Path {
            format,
            keys,
            after: self.after,
        })
    }

    fn stock(name: &str) -> Option<Self> {
        match name {
            "cargo" => Some(Self {
                format: Some("toml".into()),
                keys: Some(vec![
                    "workspace.package.version".into(),
                    "package.version".into(),
                ]),
                ..Self::default()
            }),
            "npm" => Some(Self {
                format: Some("json".into()),
                keys: Some(vec!["version".into()]),
                ..Self::default()
            }),
            "pyproject" => Some(Self {
                format: Some("toml".into()),
                keys: Some(vec!["project.version".into()]),
                ..Self::default()
            }),
            "go" => Some(Self {
                format: Some("gomod".into()),
                ..Self::default()
            }),
            "version" => Some(Self {
                format: Some("version".into()),
                ..Self::default()
            }),
            _ => None,
        }
    }

    fn merge(self, over: Self) -> Self {
        Self {
            format: over.format.or(self.format),
            keys: over.keys.or(self.keys),
            read: over.read.or(self.read),
            write: over.write.or(self.write),
            after: over.after.or(self.after),
        }
    }
}

/// One released package: its manifest, how that manifest holds the version, and
/// how it is published.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct PackageSpec {
    /// The name fragments, tags, and the changelog use for the package.
    pub name: String,
    /// The manifest, relative to the directory holding `.ctl/`.
    pub path: PathBuf,
    /// A `drivers` entry or stock driver name. Omit to infer it from the
    /// manifest file name.
    pub driver: Option<String>,
    /// Stock `cargo` / `bun`, a `publishers` key, or omit to infer. `none`
    /// releases the package on GitHub only: tag, Release, and assets, with no
    /// registry upload.
    pub publisher: Option<String>,
    /// Pretty noun (`crate`, `package`, `wheel`). Defaults from the publisher.
    pub noun: Option<String>,
    /// How the package is published, beyond what its manifest implies.
    ///
    /// A mapping holds options of the publisher the package resolves to,
    /// which keeps its upload command and its registry check: `verify: false`
    /// for cargo. A list replaces the whole upload command, and the registry
    /// check goes with it unless `query` is set. When omitted, the publisher
    /// runs with its defaults.
    #[serde(default)]
    pub publish: Option<Publish>,
    /// Fills the publisher's `{registry}` placeholder and is named in the plan.
    /// Empty means none.
    pub registry: Option<String>,
    /// Asks the registry whether a version is published, in place of the
    /// publisher's query or the crates.io index read. When omitted, the
    /// publisher decides; an empty list turns the check off for this package.
    pub query: Option<Vec<String>>,
    /// Package CHANGELOG.md. Defaults to next to the manifest.
    pub changelog: Option<PathBuf>,
    /// Lets this package leave `0.0.0` with a `minor` fragment, so its first
    /// release is `0.1.0`. When omitted, a package at `0.0.0` takes only
    /// `patch` and first releases `0.0.1`. Inert once the package has released.
    #[serde(default)]
    pub first_minor: bool,
    /// The tag of the candidate versions this package releases, such as `rc`:
    /// `verctl version` writes `<base>-<tag>.<count>` while it is set. When
    /// omitted, the package releases stable versions, and a current prerelease
    /// version becomes the stable one.
    pub prerelease: Option<Prerelease>,
    /// The version is on one marker line of the file, such as
    /// `# version: 1.2.0`, `// version: 1.2.0`, or `<!-- version: 1.2.0 -->`.
    /// So a file of any type can be a package, such as one reusable workflow
    /// or one script.
    ///
    /// When unset, a file is read by its stock or named driver, and a stray
    /// marker line in it means nothing. It takes no `driver` or driver fields,
    /// and with no publisher the package releases on GitHub only.
    #[serde(default)]
    pub marker: bool,
    /// Inline driver fields. They override the named driver's.
    #[serde(flatten)]
    pub spec: DriverSpec,
    /// Every key under the package that no field above reads. A struct that
    /// flattens another cannot deny unknown fields, so this map collects them
    /// and `no_unknown_package_keys` refuses the load when it holds one.
    #[serde(flatten)]
    #[expect(
        clippy::zero_sized_map_values,
        reason = "the keys are all that is read, and serde has no set to flatten into"
    )]
    unknown: BTreeMap<String, serde::de::IgnoredAny>,
}

/// The tag of a candidate version: lowercase ASCII letters.
///
/// Letters keep `<tag>.<count>` a valid semver prerelease with a separate
/// numeric count, keep the tag a valid npm dist-tag, and make tags compare in
/// semver order as words do (`alpha < beta < rc`). A package's rule in
/// `a_prerelease_is_lowercase_letters` refuses any other tag at load, so a
/// tag read from a loaded config always passes [`Prerelease::is_valid`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct Prerelease(String);

impl Prerelease {
    /// A tag, or the reason `tag` is not one.
    ///
    /// # Errors
    ///
    /// Returns an error when `tag` is empty or holds anything but lowercase
    /// ASCII letters.
    pub fn new(tag: &str) -> Result<Self> {
        let tag = Self(tag.to_owned());
        ensure!(tag.is_valid(), "{}", Self::RULE);
        Ok(tag)
    }

    /// The tag as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether the tag is one or more lowercase ASCII letters, and neither `x`
    /// nor `latest`.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.0.is_empty()
            && !matches!(self.0.as_str(), "x" | "latest")
            && self.0.bytes().all(|byte| byte.is_ascii_lowercase())
    }

    /// Two words are refused besides the letter rule. npm reads `x` alone as
    /// the version range "any" and refuses it as a dist-tag. `latest` is the
    /// dist-tag every plain install resolves, so a candidate under it would
    /// reach every consumer.
    const RULE: &'static str = "a prerelease tag is lowercase ASCII letters only, such as rc, beta, or next; never x, which npm reads as a version range, and never latest, which every plain install resolves; remove digits, dots, and capitals";
}

impl PackageSpec {
    /// The upload command this package wrote in place of its publisher's.
    #[must_use]
    pub fn publish_command(&self) -> Option<&[String]> {
        match &self.publish {
            Some(Publish::Command(argv)) => Some(argv),
            Some(Publish::Options(_)) | None => None,
        }
    }

    /// The options this package wrote for its publisher.
    #[must_use]
    pub fn publish_options(&self) -> Option<&PublishOptions> {
        match &self.publish {
            Some(Publish::Options(options)) => Some(options),
            Some(Publish::Command(_)) | None => None,
        }
    }

    /// The package changelog, resolved against `root`.
    #[must_use]
    pub fn changelog_path(&self, root: &Path) -> PathBuf {
        if let Some(path) = &self.changelog {
            return root.join(path);
        }
        root.join(&self.path)
            .parent()
            .unwrap_or(root)
            .join("CHANGELOG.md")
    }

    /// The version driver. It is the stock or named driver with this package's
    /// own fields over it. A lockfile follow-up is inferred when none is set.
    ///
    /// # Errors
    ///
    /// Returns an error when the merged driver is incomplete or invalid, as
    /// described for [`DriverSpec::into_driver`]. A package whose file is
    /// not `Cargo.toml`, `package.json`, `pyproject.toml`, `go.mod`, or
    /// `VERSION` needs a driver of its own.
    pub fn resolve(&self, config: &Config, root: &Path) -> Result<Driver> {
        if self.marker {
            return Ok(Driver::Text {
                text: Text::Marker,
                after: self.spec.after.clone(),
            });
        }
        let inferred = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(stock_name_for_file);
        let driver_name = self.driver.as_deref().or(inferred);
        let mut spec = driver_name
            .and_then(|name| config.driver_spec(name))
            .unwrap_or_default();
        spec = spec.merge(self.spec.clone());
        if spec.after.is_none() {
            spec.after = crate::detect::follow_up(&root.join(&self.path));
        }
        spec.into_driver(driver_name.unwrap_or(self.name.as_str()))
    }

    /// The version this package's file holds now, read through its driver.
    /// Every command that reads a current version reads it here, so each
    /// error names the package and its file.
    ///
    /// # Errors
    ///
    /// Returns an error naming the package when its driver cannot be
    /// resolved, its file cannot be read, or the driver finds no version in
    /// it.
    pub fn current(&self, config: &Config, root: &Path) -> Result<Current> {
        let path = root.join(&self.path);
        let context = || format!("package {}: {}", self.name, path.display());
        let driver = self.resolve(config, root).with_context(context)?;
        let raw = driver.source(&path).with_context(context)?;
        let version = driver.read(&raw).with_context(context)?.trim().to_owned();
        Ok(Current {
            driver,
            path,
            raw,
            version,
        })
    }
}

/// Publisher options as a package wrote them: each key with its value, not
/// yet read by the publisher they belong to.
pub type PublishOptions = BTreeMap<String, serde_json::Value>;

/// What a package wrote under `publish`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Publish {
    /// Options of the publisher the package resolves to. The publisher reads
    /// them, so an option another publisher has is an error at load.
    Options(PublishOptions),
    /// The whole upload command, run in place of the publisher's.
    Command(Vec<String>),
}

/// Untagged reads both shapes and then reports "matched no variant", naming a
/// type nobody wrote. So the shape is read here and reported in the words a
/// repo uses.
const PUBLISH_SHAPES: &str =
    "a mapping of publisher options, or a list that is the whole upload command";

impl<'de> Deserialize<'de> for Publish {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(PublishVisitor)
    }
}

struct PublishVisitor;

impl<'de> serde::de::Visitor<'de> for PublishVisitor {
    type Value = Publish;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{PUBLISH_SHAPES}")
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut parts: A) -> Result<Publish, A::Error> {
        let mut argv = Vec::new();
        while let Some(part) = parts.next_element::<String>()? {
            argv.push(part);
        }
        Ok(Publish::Command(argv))
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut entries: A) -> Result<Publish, A::Error> {
        let mut options = PublishOptions::new();
        while let Some((key, value)) = entries.next_entry::<String, serde_json::Value>()? {
            options.insert(key, value);
        }
        Ok(Publish::Options(options))
    }
}

/// A package's version as its file holds it now, from
/// [`PackageSpec::current`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Current {
    /// The driver that read the version and writes the next one.
    pub driver: Driver,
    /// The version file, joined to the repository root.
    pub path: PathBuf,
    /// The file's text: empty for a `VERSION` file that does not exist yet.
    pub raw: String,
    /// The version, trimmed.
    pub version: String,
}

/// One machine, declared once under `runners`. The key names it; this
/// is how GitHub finds it. Every label is required at once, so a three-label
/// runner is one machine carrying three labels, not three machines.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Validate)]
#[serde(deny_unknown_fields)]
#[garde(context(Config))]
#[non_exhaustive]
pub struct Runner {
    /// Every label at once. A machine with none cannot be found at all.
    #[garde(custom(at_least_one("label")))]
    pub labels: Vec<String>,
}

/// One job, declared under `ci`. The key names it; `runners` names the
/// machines from `runners` it runs on, one check each. No `os`/`arch`/
/// `triple`: validation runs a machine's checks, it does not cross-compile.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Job {
    /// Names from `runners` this job runs on. Omit for one `ubuntu-latest`
    /// machine.
    pub runners: Option<Vec<String>>,
}

/// Native GitHub Release tarballs, plus the recipe every target shares. Omit
/// for libraries. Each named sub-mapping is one target.
///
/// A tarball holds the one `binary`, or the `files` packed from `dir`. Use
/// `files` when a release needs more than its binary, such as a second
/// executable, a service unit, or a licence. A target that ships other files
/// lists its own `files`, which replace the shared list for that target.
#[derive(Debug, Clone, Deserialize, Default, Validate)]
#[garde(context(Config))]
#[garde(allow_unvalidated)]
#[non_exhaustive]
pub struct Assets {
    /// The binary name. Defaults to the first package's name.
    pub bin: Option<String>,
    /// Ran once per target before `build`. Stock rust recipe uses rustup.
    pub prepare: Option<Vec<String>>,
    /// How to produce the binary. Stock rust recipe is `cargo build --release`.
    pub build: Option<Vec<String>>,
    /// Path to the built binary, with `{bin}`, `{triple}`, `{os}`, and
    /// `{arch}`. Omit for `target/{triple}/release/{bin}`. Unused when `dir`
    /// and `files` say what to pack, so it may not be set beside them.
    #[garde(custom(binary_is_not_beside_files))]
    pub binary: Option<String>,
    /// The directory `files` are packed from, relative to the directory
    /// holding `.ctl/`, with the same placeholders as `binary`. Omit, with
    /// `files`, to pack the one `binary`.
    #[serde(default)]
    #[garde(custom(dir_comes_with_files), inner(custom(inside_the_repo)))]
    pub dir: Option<PathBuf>,
    /// Paths under `dir` packed into the tarball of every target that lists
    /// none of its own, with the same placeholders. Each keeps its place under
    /// `dir`. A directory is packed whole. Omit, with `dir`, to pack the one
    /// `binary`, or when every target lists its own.
    #[serde(default)]
    #[garde(custom(files_come_with_dir), inner(custom(inside_the_repo)))]
    pub files: Vec<PathBuf>,
    /// Every remaining sub-mapping: `assets.linux-x64` and friends. A typo'd
    /// recipe key lands here and fails as an unknown target, loudly. The
    /// map is flattened, so garde names a target's field
    /// `assets.targets.<id>`, though the file has no `targets` key.
    /// [`Config::load`] declares this field to ctl-core as flattened,
    /// so a problem names `assets.<id>` and lands on that key's line.
    #[serde(flatten)]
    #[garde(dive)]
    pub targets: BTreeMap<String, AssetTarget>,
}

/// One build target: the platform it produces and the single machine that
/// builds it. One tarball, one machine, so `runner` is not a list.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default, Validate)]
#[serde(deny_unknown_fields)]
#[garde(context(Config))]
#[garde(allow_unvalidated)]
#[non_exhaustive]
pub struct AssetTarget {
    /// Name from `runners` of the machine that builds this target.
    pub runner: Option<String>,
    /// Operating system name. Defaults from a known target id.
    pub os: Option<String>,
    /// CPU architecture name. Defaults from a known target id.
    pub arch: Option<String>,
    /// Target triple. Defaults from a known target id.
    pub triple: Option<String>,
    /// The paths this target's tarball holds under `assets.dir`, in place of
    /// `assets.files`, with the same placeholders. Use it when a platform
    /// ships other files, such as a tool that builds only on Linux. Unset,
    /// the target packs `assets.files`; an empty list would pack nothing.
    #[serde(default)]
    #[garde(
        custom(target_files_come_with_dir),
        inner(custom(at_least_one("file")), inner(custom(inside_the_repo)))
    )]
    pub files: Option<Vec<PathBuf>>,
}

/// Release assets built from someone else's source, at that source's version,
/// such as a library built from a pinned upstream release. Mise can then
/// install it as a `github:` tool. The key is the set's `{name}`. Fragments
/// never bump it, and `release` never reads it; `status --vendored <name>`
/// plans it, and `release asset <target> --vendored <name>` builds and uploads
/// it.
#[derive(Debug, Clone, Deserialize, Validate)]
#[serde(deny_unknown_fields)]
#[garde(context(Config))]
#[garde(allow_unvalidated)]
#[non_exhaustive]
pub struct Vendored {
    /// The upstream version, written by hand: `{version}` in every template.
    #[garde(
        custom(cannot_be_empty("it names the release and the archives")),
        custom(one_plain_name)
    )]
    pub version: String,
    /// Release tag, with `{name}` and `{version}`. Omit for
    /// `{name}-{version}`.
    #[serde(default = "Vendored::default_tag")]
    #[garde(
        custom(cannot_be_empty("it names the release")),
        custom(only_placeholders(&["{name}", "{version}"]))
    )]
    pub tag: String,
    /// Archive file name, with `{name}`, `{version}`, and `{id}`. Omit for
    /// `{name}-{version}-{id}.tar.gz`.
    #[serde(default = "Vendored::default_archive")]
    #[garde(
        custom(one_archive_per_target),
        custom(only_placeholders(&["{name}", "{version}", "{id}"]))
    )]
    pub archive: String,
    /// Run once per target before packing. It uses no shell, and `{name}`,
    /// `{version}`, and `{id}` are replaced. Omit when an earlier workflow step
    /// builds.
    #[serde(default)]
    pub build: Vec<String>,
    /// The directory the files are packed from, relative to the directory
    /// holding `.ctl/`, with the same placeholders.
    #[garde(custom(inside_the_repo))]
    pub dir: PathBuf,
    /// Paths under `dir` packed into the archive, each kept at its place under
    /// `dir`. A directory is packed whole.
    #[garde(custom(at_least_one("file")), inner(custom(inside_the_repo)))]
    pub files: Vec<PathBuf>,
    /// The Release body, a file relative to the directory holding `.ctl/`.
    /// Omit for a Release with no body.
    #[garde(inner(custom(inside_the_repo)))]
    pub notes: Option<PathBuf>,
    /// One entry per platform; the key is `{id}`, such as `linux-x64`.
    #[garde(custom(at_least_one_target), custom(every_key_is_one_plain_name), dive)]
    pub targets: BTreeMap<String, VendoredTarget>,
}

impl Vendored {
    fn default_tag() -> String {
        "{name}-{version}".to_owned()
    }

    fn default_archive() -> String {
        "{name}-{version}-{id}.tar.gz".to_owned()
    }
}

/// One platform of a vendored set and the single machine that builds it.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Validate)]
#[serde(deny_unknown_fields)]
#[garde(context(Config))]
#[non_exhaustive]
pub struct VendoredTarget {
    /// Name from `runners` of the machine that builds this target. verctl
    /// has no built-in vendored platform, so there is no default machine.
    #[garde(custom(runner_is_declared))]
    pub runner: String,
}

/// Two targets under one archive name would overwrite each other, and `tar
/// czf` writes only a gzipped tarball.
fn one_archive_per_target(archive: &str, config: &Config) -> garde::Result {
    ctl_core::validate::one_file_name(archive, config)?;
    if !archive.contains("{id}") {
        return Err(garde::Error::new(
            "must contain {id}, or every target writes the same archive",
        ));
    }
    if !archive.ends_with(".tar.gz") {
        return Err(garde::Error::new(
            "must end in .tar.gz, the only archive verctl writes",
        ));
    }
    Ok(())
}

/// A brace left after the known placeholders is a typo that would reach a tag
/// or a file name unchanged. The tag takes no `{id}`, since every target
/// uploads to one Release.
fn only_placeholders(allowed: &'static [&'static str]) -> impl Fn(&str, &Config) -> garde::Result {
    move |template, _| {
        let rest = allowed
            .iter()
            .fold(template.to_owned(), |rest, placeholder| {
                rest.replace(placeholder, "")
            });
        if rest.contains(['{', '}']) {
            return Err(garde::Error::new(format!(
                "has a '{{' or '}}' that is not {}",
                allowed.join(", ")
            )));
        }
        Ok(())
    }
}

/// `{name}`, `{version}`, and `{id}` are substituted into `archive` and
/// `dir` after the load checked those templates. A value holding a path
/// separator or a parent step would therefore move the archive or the packed
/// directory outside the repository.
fn one_plain_name(value: &str, _: &Config) -> garde::Result {
    if value.is_empty() || value == "." || value == ".." || value.contains(['/', '\\']) {
        return Err(garde::Error::new(format!(
            "{value:?} must be one plain name, with no / or \\ and not . or .."
        )));
    }
    Ok(())
}

fn every_key_is_one_plain_name<V>(map: &BTreeMap<String, V>, config: &Config) -> garde::Result {
    map.keys().try_for_each(|key| one_plain_name(key, config))
}

fn at_least_one_target(targets: &BTreeMap<String, VendoredTarget>, _: &Config) -> garde::Result {
    if targets.is_empty() {
        return Err(garde::Error::new("must declare at least one target"));
    }
    Ok(())
}

/// `dir` without `files` would pack nothing for a target that lists none of
/// its own, so `dir` comes with the shared `files` or a list on every target.
#[expect(
    clippy::ref_option,
    reason = "garde passes the field as it is declared"
)]
fn dir_comes_with_files(dir: &Option<PathBuf>, config: &Config) -> garde::Result {
    let packs_files = config.assets.as_ref().is_some_and(|assets| {
        !assets.files.is_empty()
            || (!assets.targets.is_empty()
                && assets.targets.values().all(|target| target.files.is_some()))
    });
    if dir.is_some() && !packs_files {
        return Err(garde::Error::new(
            "must come with files, the paths packed from it, or a files list on every target",
        ));
    }
    Ok(())
}

/// A target's `files` are packed from `assets.dir`, so they need it.
#[expect(
    clippy::ref_option,
    reason = "garde passes the field as it is declared"
)]
fn target_files_come_with_dir(files: &Option<Vec<PathBuf>>, config: &Config) -> garde::Result {
    let has_dir = config
        .assets
        .as_ref()
        .is_some_and(|assets| assets.dir.is_some());
    if files.is_some() && !has_dir {
        return Err(garde::Error::new(
            "must come with assets.dir, the directory they are packed from",
        ));
    }
    Ok(())
}

/// `files` without `dir` have no place to be packed from.
fn files_come_with_dir(files: &[PathBuf], config: &Config) -> garde::Result {
    let has_dir = config
        .assets
        .as_ref()
        .is_some_and(|assets| assets.dir.is_some());
    if !files.is_empty() && !has_dir {
        return Err(garde::Error::new(
            "must come with dir, the directory they are packed from",
        ));
    }
    Ok(())
}

/// A `binary` beside `dir` and `files`, shared or on a target, would never
/// be read, and a reader would take it for what the tarball holds. The
/// message names the first target whose own `files` conflict, so a config
/// with several targets points at one list.
#[expect(
    clippy::ref_option,
    reason = "garde passes the field as it is declared"
)]
fn binary_is_not_beside_files(binary: &Option<String>, config: &Config) -> garde::Result {
    let (Some(_), Some(assets)) = (binary, config.assets.as_ref()) else {
        return Ok(());
    };
    if let Some(id) = assets
        .targets
        .iter()
        .find_map(|(id, target)| target.files.is_some().then_some(id))
    {
        return Err(garde::Error::new(format!(
            "must be omitted when assets.{id}.files says what the tarball holds; list the binary in files",
        )));
    }
    if assets.dir.is_some() || !assets.files.is_empty() {
        return Err(garde::Error::new(
            "must be omitted when dir and files say what the tarball holds; list the binary in files",
        ));
    }
    Ok(())
}

fn runner_is_declared(runner: &str, config: &Config) -> garde::Result {
    if config.runners.contains_key(runner) {
        return Ok(());
    }
    let declared = config.runners.keys().cloned().collect::<Vec<_>>();
    Err(garde::Error::new(format!(
        "{runner:?} is not declared in runners ({})",
        if declared.is_empty() {
            "none declared".to_owned()
        } else {
            format!("declared: {}", declared.join(", "))
        }
    )))
}

/// A config that releases no package and vendors nothing has nothing to do.
fn something_to_release(packages: &[PackageSpec], config: &Config) -> garde::Result {
    if packages.is_empty() && config.vendored.is_empty() {
        return Err(garde::Error::new(
            "must declare at least one package, or a vendored set",
        ));
    }
    Ok(())
}

/// The whole file, and the context every rule in it is checked against. A
/// pattern is only unused, and an id only unknown, relative to the rest of the
/// config. `Config::parse` reads the shape with serde and then validates it
/// once, here, so a complaint names the field that has to change.
///
/// A section verctl does not know stops the load instead of being dropped.
/// Ignoring `pattern:` for `patterns:` would ship a release with the pins it
/// declares never rewritten.
#[derive(Debug, Clone, Deserialize, Default, Validate)]
#[serde(deny_unknown_fields)]
#[garde(context(Self as config))]
#[garde(allow_unvalidated)]
#[non_exhaustive]
pub struct Config {
    /// Custom drivers by name. A stock name (`cargo`, `npm`, `pyproject`,
    /// `go`, `version`) is overridden field by field.
    #[serde(default)]
    pub drivers: BTreeMap<String, DriverSpec>,
    /// What this repo releases. Omit only when the repo declares `vendored`;
    /// a config with neither has nothing to do.
    #[serde(default)]
    #[garde(
        custom(something_to_release),
        custom(no_unknown_package_keys),
        custom(release_only_names_no_registry),
        custom(publish_options_fit_the_publisher),
        custom(python_names_its_publisher),
        custom(a_marker_names_no_driver),
        custom(a_prerelease_is_lowercase_letters)
    )]
    pub packages: Vec<PackageSpec>,
    /// Publish recipes by name, for a package's `publisher` to select.
    #[serde(default)]
    #[garde(custom(none_is_not_a_publisher))]
    pub publishers: BTreeMap<String, PublisherSpec>,
    /// Machines, declared once and named. Entirely repo-owned: verctl ships no
    /// built-in names.
    #[serde(default)]
    #[garde(dive)]
    pub runners: BTreeMap<String, Runner>,
    /// Release tarball recipe. Omit for libraries.
    #[serde(default)]
    #[garde(dive)]
    pub assets: Option<Assets>,
    /// Asset sets built from someone else's source at its own version, by
    /// name. Omit when the repo vendors nothing.
    #[serde(default)]
    #[garde(custom(every_key_is_one_plain_name), dive)]
    pub vendored: BTreeMap<String, Vendored>,
    /// PR and push validation. Empty means one `verify` on `ubuntu-latest`.
    #[serde(default)]
    pub ci: BTreeMap<String, Job>,
    /// Work done after the version bumps.
    #[serde(default)]
    #[garde(custom(no_version_label))]
    pub prepare: Prepare,
    /// The Version PR's title and commit message. Omit for
    /// `chore(release): version packages` as both.
    #[serde(default)]
    pub proposal: Proposal,
    /// Whose names changelog entries never credit. Omit to list no one.
    #[serde(default)]
    pub changelog: Changelog,
    /// Where served-file templates live. Omit for the convention.
    #[serde(default)]
    #[garde(dive)]
    pub templates: Templates,
    /// How release tags are named. Omit for `v{version}`: one tag when every
    /// released package shares a version. Put `{name}` in the template for one
    /// tag (and one Release) per package.
    #[serde(default)]
    #[garde(dive)]
    pub tags: Tags,
    /// Version spellings, named once and listed by the files that carry them,
    /// so a spelling three files share is written once.
    #[serde(default)]
    #[garde(custom(every_pattern_says_where_the_version_goes))]
    #[garde(custom(every_pattern_bounds_the_version))]
    #[garde(custom(every_pattern_is_listed))]
    pub patterns: BTreeMap<String, PinPattern>,
    /// Collocated tool pins rewritten when `package` is bumped.
    #[serde(default)]
    #[garde(dive)]
    pub pins: Vec<Pin>,
}

/// Where the version goes, said once: no placeholder is a spelling that tracks
/// nothing, two is a spelling with no single version to put anywhere.
fn every_pattern_says_where_the_version_goes(
    patterns: &BTreeMap<String, PinPattern>,
    _: &Config,
) -> garde::Result {
    for (id, pattern) in patterns {
        let found = pattern.r#match.matches(PLACEHOLDER).count();
        if found != 1 {
            return Err(garde::Error::new(format!(
                "{id}: match says {PLACEHOLDER} {found} times, and must say it once"
            )));
        }
    }
    Ok(())
}

/// Where the version starts and ends, as the pattern declares it.
/// `{version}` is a version verctl writes: digits and dots, then an optional
/// candidate prerelease `-<tag>.<count>`. Without literal text on each side
/// that a version cannot hold, the pattern could end a version early or start
/// it late.
///
/// For example, `v{version}.5x` reads `v1.2.3.5x` as 1.2.3 and keeps `.5x`
/// after the new version. The file cannot settle it, because
/// `download/v{version}-linux` against `download/v1.2.3-linux-x64.tar.gz` is a
/// correct pin of 1.2.3.
fn every_pattern_bounds_the_version(
    patterns: &BTreeMap<String, PinPattern>,
    _: &Config,
) -> garde::Result {
    for (id, pattern) in patterns {
        let Some((before, after)) = pattern.r#match.split_once(PLACEHOLDER) else {
            continue;
        };
        if after.contains(PLACEHOLDER) {
            continue;
        }
        if let Some(problem) = unbounded(before, after, pattern.whole_line) {
            return Err(garde::Error::new(format!("{id}: {problem}")));
        }
    }
    Ok(())
}

/// What leaves one side of the placeholder open, if anything does.
///
/// A side's literal is the text between `{version}` and the pattern's edge on
/// that side. A version and its prerelease or build hold only digits, `.`,
/// `-`, and `+`. So the side is bounded when its literal holds some other
/// character. Under `whole_line` the line start or end bounds a side whose
/// literal is empty.
///
/// The character next to `{version}` also matters. A digit there, or a `.`
/// with a digit beyond it, lets the regex end a version early or start it
/// late. `v{version}.5x` reads `v1.2.3.5x` as 1.2.3, so that side is open.
fn unbounded(before: &str, after: &str, whole_line: bool) -> Option<String> {
    let sides = [
        (
            before,
            before.chars().rev().collect::<String>(),
            "before",
            "starts",
        ),
        (after, after.to_owned(), "after", "ends"),
    ];
    for (text, outward, side, edge) in sides {
        if text.is_empty() {
            if whole_line {
                continue;
            }
            return Some(format!(
                "nothing {side} {PLACEHOLDER} marks where the version {edge}: add the text {side} it to match, or set whole_line: true"
            ));
        }
        if text
            .chars()
            .all(|next| matches!(next, '0'..='9' | '.' | '-' | '+'))
        {
            return Some(format!(
                "{text:?} {side} {PLACEHOLDER} holds only digits, '.', '-', and '+', which a version can continue into: add more of the text {side} it to match"
            ));
        }
        let mut nearest = outward.chars();
        if matches!(
            (nearest.next(), nearest.next()),
            (Some('0'..='9'), _) | (Some('.'), Some('0'..='9'))
        ) {
            return Some(format!(
                "{text:?} {side} {PLACEHOLDER} puts a digit, or a '.' and a digit, next to the version, so a longer version would match only in part: put a character other than a digit or '.' next to {PLACEHOLDER}"
            ));
        }
    }
    None
}

/// A declared pattern nothing lists is dead configuration instead of a spare.
/// It reads like a tracked spelling while no file is checked against it.
fn every_pattern_is_listed(
    patterns: &BTreeMap<String, PinPattern>,
    config: &Config,
) -> garde::Result {
    let listed: BTreeSet<&str> = config
        .pins
        .iter()
        .flat_map(|pin| pin.pattern_ids.iter().map(String::as_str))
        .collect();
    for id in patterns.keys() {
        if !listed.contains(id.as_str()) {
            return Err(garde::Error::new(format!(
                "{id:?} is declared and no pin lists it"
            )));
        }
    }
    Ok(())
}

/// A file that serves a package version and must track it.
#[derive(Debug, Clone, Deserialize, Validate)]
#[serde(deny_unknown_fields)]
#[garde(context(Config as config))]
#[garde(allow_unvalidated)]
#[non_exhaustive]
pub struct Pin {
    /// The file, relative to the directory holding `.ctl/`.
    #[garde(custom(inside_the_repo))]
    pub file: PathBuf,
    /// A mise `[tools]` entry, in a file that parses as TOML. Its own
    /// `?ref=v…` includes move with it.
    pub tool: Option<String>,
    /// `patterns` ids this file lists, so the file a spelling belongs to is
    /// written down instead of implied by table placement.
    #[serde(default, rename = "patterns")]
    #[garde(
        custom(each_id_is_declared),
        custom(no_id_is_listed_twice),
        custom(something_to_rewrite(self.tool.is_some()))
    )]
    pub pattern_ids: Vec<String>,
    /// The patterns those ids name, filled in by `Config::load`, the only way
    /// to build a pin that rewrites anything.
    #[serde(skip)]
    pub patterns: Vec<PinPattern>,
    /// The package whose version this file serves.
    pub package: String,
}

/// A name is only a name if something declares it. `none` names release-only
/// packages. A `publishers.none` recipe could never run, because every package
/// that names it uploads nothing.
fn none_is_not_a_publisher(
    publishers: &BTreeMap<String, PublisherSpec>,
    _: &Config,
) -> garde::Result {
    if publishers.contains_key(crate::publisher::RELEASE_ONLY) {
        return Err(garde::Error::new(
            "publishers.none is reserved for release-only packages; rename that publisher",
        ));
    }
    Ok(())
}

/// A key under a package that nothing reads. Ignoring `publsher: none` would
/// publish a package to its registry that the file meant to release on GitHub
/// only.
fn no_unknown_package_keys(packages: &[PackageSpec], _: &Config) -> garde::Result {
    for package in packages {
        if !package.unknown.is_empty() {
            let keys: Vec<&str> = package.unknown.keys().map(String::as_str).collect();
            return Err(garde::Error::new(format!(
                "package {:?} has {} verctl does not read: {}; check the spelling",
                package.name,
                if keys.len() == 1 { "a key" } else { "keys" },
                keys.join(", ")
            )));
        }
    }
    Ok(())
}

/// A candidate tag that is not lowercase ASCII letters. `rc1` would make the
/// version `0.8.0-rc1.1`, which counts on no further, and `rc.1` would hide
/// the count inside the tag.
fn a_prerelease_is_lowercase_letters(packages: &[PackageSpec], _: &Config) -> garde::Result {
    for package in packages {
        if let Some(tag) = package.prerelease.as_ref().filter(|tag| !tag.is_valid()) {
            return Err(garde::Error::new(format!(
                "package {:?} has prerelease {:?}, which is not a tag; {}",
                package.name,
                tag.as_str(),
                Prerelease::RULE
            )));
        }
    }
    Ok(())
}

/// `publisher: none`, declared or inferred from its version source, uploads
/// nothing. A registry or an upload command on the same package would be a
/// setting nothing reads.
fn release_only_names_no_registry(packages: &[PackageSpec], config: &Config) -> garde::Result {
    for package in packages {
        if crate::publisher::publisher_name(config, package) == crate::publisher::RELEASE_ONLY
            && (package.registry.is_some() || package.publish.is_some())
        {
            return Err(garde::Error::new(format!(
                "package {:?} has publisher: none, which uploads to no registry; remove its registry and publish",
                package.name
            )));
        }
    }
    Ok(())
}

/// An option belongs to one publisher. One the package's publisher does not
/// have would be a setting nothing reads, so it fails here, in that
/// publisher's words.
fn publish_options_fit_the_publisher(packages: &[PackageSpec], config: &Config) -> garde::Result {
    for package in packages {
        if let Err(problem) = crate::publisher::check_options(config, package) {
            return Err(garde::Error::new(problem));
        }
    }
    Ok(())
}

/// `marker: true` says where the version is, so a `driver` or driver fields
/// beside it would be a second answer nothing reads. `after` is the
/// exception: it is what runs after the bump, not where the version is.
fn a_marker_names_no_driver(packages: &[PackageSpec], _: &Config) -> garde::Result {
    for package in packages {
        let spec = &package.spec;
        if package.marker
            && (package.driver.is_some()
                || spec.format.is_some()
                || spec.keys.is_some()
                || spec.read.is_some()
                || spec.write.is_some())
        {
            return Err(garde::Error::new(format!(
                "package {:?} has marker: true, which reads the version from its marker line; remove its driver, format, keys, read, and write",
                package.name
            )));
        }
    }
    Ok(())
}

/// verctl has no stock Python publisher, so a `pyproject.toml` package that
/// names none would release a tag while nothing reached the Python Package
/// Index. It names one, through `publisher`, `publish`, or a `publishers`
/// entry under its own name, or says `publisher: none` to release on GitHub
/// only.
fn python_names_its_publisher(packages: &[PackageSpec], config: &Config) -> garde::Result {
    for package in packages {
        if package.path.file_name() == Some(OsStr::new("pyproject.toml"))
            && package.publisher.is_none()
            && package.publish_command().is_none()
            && !config.publishers.contains_key(&package.name)
        {
            return Err(garde::Error::new(format!(
                "package {:?} needs a publisher: verctl has no stock Python publisher; set publisher to a publishers entry, or to none to release on GitHub only",
                package.name
            )));
        }
    }
    Ok(())
}

fn each_id_is_declared(ids: &[String], config: &Config) -> garde::Result {
    for id in ids {
        if !config.patterns.contains_key(id) {
            return Err(garde::Error::new(format!("no patterns.{id}")));
        }
    }
    Ok(())
}

/// Listing a spelling twice does not set a count: arity belongs to the pattern.
fn no_id_is_listed_twice(ids: &[String], _: &Config) -> garde::Result {
    let mut seen = BTreeSet::new();
    for id in ids {
        if !seen.insert(id.as_str()) {
            return Err(garde::Error::new(format!("lists {id:?} twice")));
        }
    }
    Ok(())
}

/// A pin with neither a `tool` nor a pattern names a file and asks for nothing.
fn something_to_rewrite(has_tool: bool) -> impl Fn(&[String], &Config) -> garde::Result {
    move |ids, _| {
        if has_tool || !ids.is_empty() {
            return Ok(());
        }
        Err(garde::Error::new("needs a tool or a pattern to rewrite"))
    }
}

/// The directory every ctl CLI shares, so a repo root gains one entry however
/// many of them it uses. verctl owns the files it names here and nothing else.
pub const DIR: &str = ".ctl";

/// verctl's declarations, named after the task an operator runs (`mise run
/// ver`) rather than after the crate that reads them.
pub const FILE: &str = ".ctl/ver.yaml";

/// The directory a config governs: what every `path` in it is relative to.
///
/// Declarations sit in `<root>/.ctl/`, so the directory holding the file is a
/// level too deep and its parent is the root. `-c crates/foo/.ctl/ver.yaml`
/// therefore still governs `crates/foo`. A file outside a `.ctl` directory is
/// read where it sits, which is what an ad-hoc `-c` asks for.
#[must_use]
pub fn root_of(config: &Path) -> PathBuf {
    let dir = here(config.parent());
    if dir.file_name() == Some(OsStr::new(DIR)) {
        return here(dir.parent()).to_path_buf();
    }
    dir.to_path_buf()
}

fn here(path: Option<&Path>) -> &Path {
    path.filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

/// What a `pins` pattern puts where the version goes.
pub const PLACEHOLDER: &str = "{version}";

/// What a tag template puts where the package name goes.
pub const NAME_PLACEHOLDER: &str = "{name}";

/// How release tags are named. The arity falls out of the template: no
/// `{name}` means one tag for the release; `{name}` means one tag per package.
#[derive(Debug, Clone, Deserialize, Validate)]
#[serde(deny_unknown_fields)]
#[garde(context(Config))]
#[non_exhaustive]
pub struct Tags {
    /// Literal text with `{version}` and optionally `{name}`. Defaults to
    /// `v{version}`, so a repo that writes nothing keeps today's behaviour.
    #[serde(default = "Tags::default_template")]
    #[garde(
        custom(cannot_be_empty(
            "it is the shape of every release tag, like \"v{version}\" or \"{name}@{version}\""
        )),
        custom(tag_template_placeholders)
    )]
    pub template: String,
}

impl Tags {
    fn default_template() -> String {
        format!("v{PLACEHOLDER}")
    }

    /// Whether the template names the package, so each package gets its own
    /// tag.
    #[must_use]
    pub fn uses_name(&self) -> bool {
        self.template.contains(NAME_PLACEHOLDER)
    }

    /// Substitute `{version}` and, when given, `{name}`. The load already
    /// proved those are the only braces, so a leftover `{` here is a bug.
    ///
    /// # Errors
    ///
    /// Returns an error when the template needs `{name}` and `name` is `None`,
    /// or when braces remain after substitution.
    pub fn render(&self, version: &str, name: Option<&str>) -> Result<String> {
        let mut out = self.template.replace(PLACEHOLDER, version);
        if let Some(name) = name {
            out = out.replace(NAME_PLACEHOLDER, name);
        } else if out.contains(NAME_PLACEHOLDER) {
            bail!(
                "tags.template {:?} needs {NAME_PLACEHOLDER} but no package name was given",
                self.template
            );
        }
        ensure!(
            !out.contains('{') && !out.contains('}'),
            "tags.template {:?} still has braces after substitution",
            self.template
        );
        Ok(out)
    }
}

impl Default for Tags {
    fn default() -> Self {
        Self {
            template: Self::default_template(),
        }
    }
}

/// A tag template may only say `{version}` and `{name}`, must say at least one
/// of them, and must say `{version}`. A name with no version cannot date a
/// release.
fn tag_template_placeholders(template: &str, _: &Config) -> garde::Result {
    let mut saw_version = false;
    let mut saw_name = false;
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            return Err(garde::Error::new("has an opening '{' with no closing '}'"));
        };
        let name = &after[..end];
        match name {
            "version" => saw_version = true,
            "name" => saw_name = true,
            _ => {
                return Err(garde::Error::new(format!(
                    "names {{{name}}}, and the only placeholders are {PLACEHOLDER} and {NAME_PLACEHOLDER}"
                )));
            }
        }
        rest = &after[end + 1..];
    }
    if !saw_version && !saw_name {
        return Err(garde::Error::new(format!(
            "has no placeholder — put {PLACEHOLDER} where the version goes"
        )));
    }
    if !saw_version {
        return Err(garde::Error::new(format!(
            "must say {PLACEHOLDER} — a tag with only {NAME_PLACEHOLDER} cannot date a release"
        )));
    }
    let leftover = template
        .replace(PLACEHOLDER, "")
        .replace(NAME_PLACEHOLDER, "");
    if leftover.contains('{') || leftover.contains('}') {
        return Err(garde::Error::new(format!(
            "has a '{{' or '}}' that is not {PLACEHOLDER} or {NAME_PLACEHOLDER}"
        )));
    }
    let _ = saw_name;
    Ok(())
}

/// Where the templates for served files live, and how they are marked. The
/// defaults are the whole convention: a repo that follows it writes nothing.
#[derive(Debug, Clone, Deserialize, Validate)]
#[serde(deny_unknown_fields)]
#[garde(context(Config))]
#[non_exhaustive]
pub struct Templates {
    /// The directory every template lives in, flat. Each one declares its own
    /// target, so nothing sits beside the file it generates.
    #[serde(default = "Templates::default_source")]
    #[garde(custom(inside_the_repo))]
    pub source: PathBuf,
    /// What marks a source file as a template of its target.
    #[serde(default = "Templates::default_suffix")]
    #[garde(custom(cannot_be_empty(
        "it is what marks a file in the source tree as a template, like \".jinja\""
    )))]
    pub suffix: String,
}

impl Templates {
    fn default_source() -> PathBuf {
        Path::new(DIR).join("templates")
    }

    fn default_suffix() -> String {
        ".jinja".to_owned()
    }
}

impl Default for Templates {
    fn default() -> Self {
        Self {
            source: Self::default_source(),
            suffix: Self::default_suffix(),
        }
    }
}

/// One version spelling in a file, and how often the file may say it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct PinPattern {
    /// The text around a version, `{version}` where the version is. Literal
    /// text instead of a regex, so nothing about the file's format is
    /// assumed. Exactly one placeholder, checked over `patterns` so the
    /// complaint names the id.
    ///
    /// A side is bounded when the text between `{version}` and the pattern's
    /// edge holds a character that is not a digit, `.`, `-`, or `+`. The
    /// character next to `{version}` must not be a digit, or a `.` with a
    /// digit beyond it.
    ///
    /// Under `whole_line` the line start or end bounds a side whose literal is
    /// empty. The load refuses a pattern with an open side.
    pub r#match: String,
    /// How often the file must say it. A pin that lost its only mention, or
    /// drifted to one nobody declared, fails the release rather than serving
    /// a stale version.
    #[serde(default)]
    pub occurrences: Occurrences,
    /// Whether the match is a whole line rather than text inside one.
    ///
    /// When unset, a pattern matches anywhere, which is what an inline
    /// spelling wants: `github:org/tool@1.2.3` is inside a longer line.
    ///
    /// A pattern that owns its line sets this flag instead of anchoring on
    /// whatever text happens to be next to it. The alternative is spelling the
    /// neighbour into `match`, which then breaks when the neighbour moves.
    ///
    /// The flag also makes a partial version fail instead of rewriting,
    /// because a line ending in `0.1.1_local` does not end where the version
    /// does.
    #[serde(default)]
    pub whole_line: bool,
}

/// How many times a pattern must match. A count is not always the useful
/// shape. A document whose examples come and go needs "one or more".
/// Retiring a spelling needs "none".
#[derive(Debug, Clone, Copy, Default)]
#[non_exhaustive]
pub enum Occurrences {
    /// Exactly one. What a pin normally is, so it is the default.
    #[default]
    Once,
    /// One or more, however many. For a file whose mentions grow and shrink
    /// on their own, where only "still tracked" is worth asserting.
    Many,
    /// None. A spelling that must stay gone, so its return fails a release
    /// instead of being served.
    Never,
    /// Exactly this many, for a file that says it a known number of times.
    Exactly(usize),
    /// This many or more.
    AtLeast(usize),
}

/// The vocabulary, spelled out. A derived enum cannot read both shapes a repo
/// writes. An unbounded word is a plain scalar. A bound is a mapping that a
/// newtype variant would only accept as a YAML tag (`!exactly 2`).
///
/// Untagged reads both and then reports "matched no variant", naming a type
/// nobody wrote. So the arity is read here and reports errors in the words a
/// repo uses.
const ARITY: &str = "once, many, never, {exactly: N}, or {at_least: N}";

impl<'de> Deserialize<'de> for Occurrences {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(OccurrencesVisitor)
    }
}

struct OccurrencesVisitor;

impl<'de> serde::de::Visitor<'de> for OccurrencesVisitor {
    type Value = Occurrences;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{ARITY}")
    }

    fn visit_str<E: serde::de::Error>(self, word: &str) -> Result<Occurrences, E> {
        match word {
            "once" => Ok(Occurrences::Once),
            "many" => Ok(Occurrences::Many),
            "never" => Ok(Occurrences::Never),
            _ => Err(E::custom(format!("must be {ARITY}"))),
        }
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(
        self,
        mut bound: A,
    ) -> Result<Occurrences, A::Error> {
        use serde::de::Error;
        let wrong = || A::Error::custom(format!("must be {ARITY}"));
        let key: String = bound.next_key()?.ok_or_else(wrong)?;
        #[expect(
            clippy::map_err_ignore,
            reason = "the report names the accepted shapes; the number parse error adds nothing"
        )]
        let count: usize = bound.next_value().map_err(|_| wrong())?;
        if bound.next_key::<serde::de::IgnoredAny>()?.is_some() {
            return Err(wrong());
        }
        match key.as_str() {
            "exactly" => Ok(Occurrences::Exactly(count)),
            "at_least" => Ok(Occurrences::AtLeast(count)),
            _ => Err(wrong()),
        }
    }
}

impl Occurrences {
    /// Whether `found` occurrences satisfy this rule.
    #[must_use]
    pub const fn allows(self, found: usize) -> bool {
        match self {
            Self::Once => found == 1,
            Self::Many => found >= 1,
            Self::Never => found == 0,
            Self::Exactly(count) => found == count,
            Self::AtLeast(count) => found >= count,
        }
    }
}

impl fmt::Display for Occurrences {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Once => formatter.write_str("exactly once"),
            Self::Many => formatter.write_str("one or more"),
            Self::Never => formatter.write_str("never"),
            Self::Exactly(count) => write!(formatter, "exactly {count} times"),
            Self::AtLeast(count) => write!(formatter, "{count} or more times"),
        }
    }
}

/// Whose names changelog entries never credit. Release notes credit no
/// maintainer, and only this list identifies who one is: GitHub's
/// `author_association` reads a private organization member as `CONTRIBUTOR`
/// through an App token.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Changelog {
    /// GitHub logins never credited: maintainers and the Apps that open pull
    /// requests. Omit to list no one.
    #[serde(default)]
    pub internal_authors: Vec<String>,
}

impl Changelog {
    /// Whether `login` is listed, ignoring case as GitHub logins do.
    #[must_use]
    pub fn is_internal(&self, login: &str) -> bool {
        self.internal_authors
            .iter()
            .any(|listed| listed.eq_ignore_ascii_case(login))
    }
}

/// Extra work after version bumps, committed on the Version PR.
///
/// Unknown keys fail the load, as in every other block.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Prepare {
    /// argv after all bumps (for example `["mise", "run", "version-sync"]`).
    pub after: Option<Vec<String>>,
    /// Extra paths `after` may write. Anything else dirty fails.
    #[serde(default)]
    pub stage: Vec<String>,
    /// Collect gitignored paths that match `stage`. Off by default.
    #[serde(default)]
    pub stage_ignored: bool,
    /// Removed: the Version PR is not labeled, since nothing reads the label.
    /// Kept only for files that still set it. Such a file fails the load with
    /// a message to delete it, instead of serde's bare unknown-field error.
    /// `true` when the key is present with any value, null included;
    /// absent means the file is current.
    #[serde(default, deserialize_with = "key_present")]
    version_label: bool,
}

/// Whether a key is present, whatever its value. serde reads an explicit null
/// into `Option::None`, the same as an absent key. An `Option` therefore holds
/// the same value for `version_label: ~` as for a file that never set it.
/// serde calls this only when the key is in the document.
fn key_present<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    serde::de::IgnoredAny::deserialize(deserializer).map(|_| true)
}

/// The Version PR's title and commit message.
///
/// Both default to what verctl wrote before the block existed, so a
/// repository that sets neither sees the same pull request.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Proposal {
    /// The pull request title. Unset or empty means
    /// [`Proposal::DEFAULT_TITLE`].
    #[serde(default)]
    pub title: Option<String>,
    /// The commit message on the Version PR branch. Unset or empty means the
    /// resolved title.
    #[serde(default)]
    pub commit_message: Option<String>,
}

impl Proposal {
    /// The title used when `title` is unset or empty.
    pub const DEFAULT_TITLE: &'static str = "chore(release): version packages";

    /// The resolved title: the configured one, or [`Proposal::DEFAULT_TITLE`].
    #[must_use]
    pub fn title(&self) -> &str {
        self.title
            .as_deref()
            .filter(|title| !title.is_empty())
            .unwrap_or(Self::DEFAULT_TITLE)
    }

    /// The resolved commit message: the configured one, or the title.
    #[must_use]
    pub fn commit_message(&self) -> &str {
        self.commit_message
            .as_deref()
            .filter(|message| !message.is_empty())
            .unwrap_or_else(|| self.title())
    }
}

/// `prepare.version_label` labeled the Version PR until nothing read the
/// label; a file that still sets it is told to delete it.
fn no_version_label(prepare: &Prepare, _: &Config) -> garde::Result {
    if prepare.version_label {
        return Err(garde::Error::new(
            "prepare.version_label is removed: the Version PR is no longer labeled; delete this key",
        ));
    }
    Ok(())
}

impl Config {
    /// Read and validate the file at `path`.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read, when its YAML does not
    /// match the schema, or when validation fails. Each complaint names the
    /// line it comes from.
    pub fn load(path: &Path) -> Result<Self> {
        Self::from_input(Input::read(path)?)
    }

    /// A config from text in memory, named as the file a repo writes.
    ///
    /// # Errors
    ///
    /// Returns an error when `raw` does not match the schema or fails
    /// validation, as described for [`Config::load`].
    pub fn parse(raw: &str) -> Result<Self> {
        Self::from_input(Input::new(".ctl/ver.yaml", raw))
    }

    /// The one parse boundary. ctl-core's declared input reads the shape and
    /// validates it once against the whole document, placing each complaint
    /// on its line. A document is therefore read exactly one way.
    ///
    /// `assets.targets` is the one flattened map whose entries garde dives
    /// into, so it is declared here, and a target's problem names the path
    /// the file holds.
    fn from_input(input: Input) -> Result<Self> {
        let input = input.flattened(FieldPath::root().key("assets").key("targets"));
        let mut config: Self = input.parse()?;
        input.check(&config, &config)?;
        config.resolve_patterns();
        Ok(config)
    }

    /// Turn every pin's `patterns` ids into the patterns they name, so no
    /// consumer looks an id up again. Validation already proved every name
    /// resolves, which is why this cannot fail.
    fn resolve_patterns(&mut self) {
        let declared = self.patterns.clone();
        for pin in &mut self.pins {
            pin.patterns = pin
                .pattern_ids
                .iter()
                .filter_map(|id| declared.get(id).cloned())
                .collect();
        }
    }

    /// The package declared as `name`.
    ///
    /// # Errors
    ///
    /// Returns an error when no declared package has that name.
    pub fn find(&self, name: &str) -> Result<&PackageSpec> {
        self.packages
            .iter()
            .find(|package| package.name == name)
            .with_context(|| format!("package {name:?} is not in verctl config"))
    }

    /// The spec for `name`: a stock driver, a declared one, or a declared one
    /// over the stock.
    #[must_use]
    pub fn driver_spec(&self, name: &str) -> Option<DriverSpec> {
        match (DriverSpec::stock(name), self.drivers.get(name)) {
            (Some(stock), Some(over)) => Some(stock.merge(over.clone())),
            (Some(stock), None) => Some(stock),
            (None, Some(over)) => Some(over.clone()),
            (None, None) => None,
        }
    }
}

fn stock_name_for_file(file_name: &str) -> Option<&'static str> {
    match file_name {
        "Cargo.toml" => Some("cargo"),
        "package.json" => Some("npm"),
        "pyproject.toml" => Some("pyproject"),
        "go.mod" => Some("go"),
        "VERSION" => Some("version"),
        _ => None,
    }
}

pub(crate) fn stock_driver(name: &str) -> Result<Driver> {
    DriverSpec::stock(name)
        .with_context(|| format!("unknown stock driver {name:?}"))?
        .into_driver(name)
}

#[cfg(test)]
mod tests {
    use indoc::{formatdoc, indoc};

    use super::*;

    /// A whole config, so what is under test is what a repo actually writes.
    fn load(body: &str) -> Result<Config> {
        Config::parse(body)
    }

    /// The one package every fixture below needs, and nothing else.
    const PACKAGE: &str = indoc! {"
        packages:
          - name: demo
            path: Cargo.toml
    "};

    /// `publisher: none` uploads nothing, so a registry or an upload command
    /// beside it fails the load instead of being ignored.
    #[test]
    fn a_release_only_package_names_no_registry() {
        let config = load(indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
                publisher: none
        "})
        .unwrap();
        assert_eq!(config.packages[0].publisher.as_deref(), Some("none"));
        let error = load(&formatdoc! {r#"
            {PACKAGE}
            publishers:
              none:
                argv: ["true"]
        "#})
        .expect_err("publishers.none is reserved");
        assert!(
            format!("{error:#}").contains("publishers.none is reserved"),
            "{error:#}"
        );
        for extra in ["registry: crates-io", "publish: [cargo, publish]"] {
            let error = load(&formatdoc! {"
                packages:
                  - name: demo
                    path: Cargo.toml
                    publisher: none
                    {extra}
            "})
            .expect_err(extra);
            assert!(
                format!("{error:#}").contains("has publisher: none"),
                "{extra}: {error:#}"
            );
        }
    }

    /// A candidate tag is lowercase letters. Another spelling fails the load,
    /// naming the package and the field.
    #[test]
    fn a_prerelease_is_lowercase_letters() {
        for tag in ["rc", "alpha", "beta", "next", "canary"] {
            let config = load(&formatdoc! {"
                packages:
                  - name: demo
                    path: Cargo.toml
                    prerelease: {tag}
            "})
            .expect(tag);
            let spec = config.find("demo").expect("demo");
            assert_eq!(spec.prerelease.as_ref().map(Prerelease::as_str), Some(tag));
        }
        for tag in ["rc1", "RC", "rc.1", "1", "x", "latest", "\"\""] {
            let error = load(&formatdoc! {"
                packages:
                  - name: demo
                    path: Cargo.toml
                    prerelease: {tag}
            "})
            .expect_err(tag);
            let message = format!("{error:#}");
            assert!(
                message.contains("package \"demo\" has prerelease")
                    && message.contains("lowercase ASCII letters"),
                "{tag}: {message}"
            );
        }
    }

    /// `marker: true` already marks where the version is. A driver beside it
    /// fails the load instead of being ignored. `after` still applies.
    #[test]
    fn a_marker_names_no_driver() {
        for extra in [
            "driver: cargo",
            "format: toml",
            "keys: [project.version]",
            "read: [cat]",
            "write: [cat]",
        ] {
            let error = load(&formatdoc! {"
                packages:
                  - name: ci
                    path: ci.yml
                    marker: true
                    {extra}
            "})
            .expect_err(extra);
            assert!(
                format!("{error:#}").contains("has marker: true"),
                "{extra}: {error:#}"
            );
        }
        load(indoc! {"
            packages:
              - name: ci
                path: ci.yml
                marker: true
                after: mise run render
        "})
        .expect("after runs beside a marker");
    }

    /// No stock recipe uploads to the Python Package Index, so a Python
    /// package names its publisher, or `none`, before anything is released.
    #[test]
    fn a_python_package_names_its_publisher() {
        let error = load(indoc! {"
            packages:
              - name: demo
                path: pyproject.toml
        "})
        .expect_err("no publisher");
        assert!(
            format!("{error:#}").contains("needs a publisher"),
            "{error:#}"
        );
        for named in [
            "publisher: none",
            "publish: [uv, publish]",
            "publisher: pypi",
        ] {
            load(&formatdoc! {"
                packages:
                  - name: demo
                    path: pyproject.toml
                    {named}
                publishers:
                  pypi:
                    argv: [uv, publish]
            "})
            .unwrap_or_else(|error| panic!("{named}: {error:#}"));
        }
        load(indoc! {"
            packages:
              - name: demo
                path: pyproject.toml
            publishers:
              demo:
                argv: [uv, publish]
        "})
        .expect("a publishers entry under the package name");
    }

    /// A `path` is relative to the repository, not to the directory the
    /// declarations happen to sit in, so `.ctl` never lands in front of it.
    #[test]
    fn the_root_is_the_directory_holding_the_ctl_dir() {
        let cases = [
            (".ctl/ver.yaml", "."),
            ("crates/foo/.ctl/ver.yaml", "crates/foo"),
            ("/srv/repo/.ctl/ver.yaml", "/srv/repo"),
            ("ver.yaml", "."),
            ("/tmp/scratch/ver.yaml", "/tmp/scratch"),
        ];
        for (config, expected) in cases {
            assert_eq!(root_of(Path::new(config)), Path::new(expected), "{config}");
        }
    }

    /// Every arity a repo can write, in the spelling it writes it.
    #[test]
    fn every_occurrence_form_parses_and_means_what_it_says() {
        let config = load(&formatdoc! {r#"
            {PACKAGE}
            patterns:
              omitted:
                match: "a@{{version}} --"
              once:
                match: "b@{{version}} --"
                occurrences: once
              many:
                match: "c@{{version}} --"
                occurrences: many
              never:
                match: "d@{{version}} --"
                occurrences: never
              twice:
                match: "e@{{version}} --"
                occurrences: {{exactly: 2}}
              floor:
                match: "f@{{version}} --"
                occurrences: {{at_least: 2}}

            pins:
              - file: README.md
                package: demo
                patterns: [omitted, once, many, never, twice, floor]
        "#})
        .unwrap();
        let allowed: Vec<Vec<usize>> = config.pins[0]
            .patterns
            .iter()
            .map(|pattern| (0..4).filter(|n| pattern.occurrences.allows(*n)).collect())
            .collect();
        assert_eq!(
            allowed,
            [
                vec![1],       // omitted: exactly once
                vec![1],       // once
                vec![1, 2, 3], // many
                vec![0],       // never
                vec![2],       // exactly 2
                vec![2, 3],    // at least 2
            ]
        );
    }

    /// A section verctl does not know is a typo. A typo that loads is a
    /// release that does none of what the section was meant to do.
    #[test]
    fn a_misspelled_section_stops_the_load() {
        let error = load(&formatdoc! {r#"
            {PACKAGE}
            pattern:
              install:
                match: "demo@{{version}} --"
        "#})
        .expect_err("pattern is not a section");
        assert!(format!("{error:#}").contains("pattern"), "{error:#}");
    }

    /// A bound is one key and its number. A second key means the arity was
    /// mistyped, and falling through to whichever variant the first key
    /// satisfies would pin a count nobody wrote.
    #[test]
    fn a_bound_with_a_second_key_is_not_the_bound_it_starts_with() {
        let error = load(&formatdoc! {r#"
            {PACKAGE}
            patterns:
              wrong:
                match: "a@{{version}} --"
                occurrences: {{exactly: 2, at_leats: 3}}

            pins:
              - file: README.md
                package: demo
                patterns: [wrong]
        "#})
        .expect_err("at_leats is not a key");
        assert!(format!("{error:#}").contains(ARITY), "{error:#}");
    }

    /// A count that is not a whole number is the same mistake as a wrong
    /// key. It is reported in the same words instead of serde's.
    #[test]
    fn a_bound_with_a_bad_count_names_the_arity() {
        for count in ["-1", "many"] {
            let error = load(&formatdoc! {r#"
                {PACKAGE}
                patterns:
                  wrong:
                    match: "a@{{version}} --"
                    occurrences: {{exactly: {count}}}

                pins:
                  - file: README.md
                    package: demo
                    patterns: [wrong]
            "#})
            .expect_err("the count is not a whole number");
            assert!(format!("{error:#}").contains(ARITY), "{count}: {error:#}");
        }
    }

    #[test]
    fn an_unknown_arity_is_not_silently_a_default() {
        let broken = load(&formatdoc! {r#"
            {PACKAGE}
            patterns:
              wrong:
                match: "a@{{version}} --"
                occurrences: sometimes

            pins:
              - file: README.md
                package: demo
                patterns: [wrong]
        "#});
        assert!(broken.is_err());
    }

    /// One spelling, named once, carried by two files: what ids are for.
    #[test]
    fn one_named_pattern_serves_every_file_that_lists_it() {
        let config = load(&formatdoc! {r#"
            {PACKAGE}
            patterns:
              install:
                match: "demo@{{version}} --"
              mention:
                match: "v{{version}} --"
                occurrences: many

            pins:
              - file: README.md
                package: demo
                patterns: [install, mention]
              - file: docs/install.md
                package: demo
                patterns: [install]
        "#})
        .unwrap();
        let carried: Vec<Vec<&str>> = config
            .pins
            .iter()
            .map(|pin| {
                pin.patterns
                    .iter()
                    .map(|pattern| pattern.r#match.as_str())
                    .collect()
            })
            .collect();
        assert_eq!(
            carried,
            [
                vec!["demo@{version} --", "v{version} --"],
                vec!["demo@{version} --"]
            ],
            "each file carries the patterns it lists, in the order it lists them"
        );
    }

    /// Each side of `{version}` needs text a version cannot hold, or the line
    /// under `whole_line`. Every refusal names the pattern and the side.
    #[test]
    fn a_pattern_marks_where_the_version_starts_and_ends() {
        let only = "holds only digits, '.', '-', and '+'";
        let cases = [
            ("name-{version}.tar.gz", false, None),
            ("download/v{version}-linux", false, None),
            ("v{version}.tar", false, None),
            ("demo@{version} --", false, None),
            ("`demo@{version}`", false, None),
            ("verctl-{version}", true, None),
            ("version: {version}", true, None),
            ("{version}", true, None),
            (
                "{version}",
                false,
                Some("nothing before {version}".to_owned()),
            ),
            (
                "verctl-{version}",
                false,
                Some("nothing after {version}".to_owned()),
            ),
            (
                "version: {version}",
                false,
                Some("skill: nothing after {version}".to_owned()),
            ),
            (
                "verctl@{version}.",
                false,
                Some(format!("\".\" after {{version}} {only}")),
            ),
            (
                "v{version}-",
                false,
                Some(format!("\"-\" after {{version}} {only}")),
            ),
            (
                "v{version}+",
                false,
                Some(format!("\"+\" after {{version}} {only}")),
            ),
            (
                "-{version}",
                false,
                Some(format!("\"-\" before {{version}} {only}")),
            ),
            (
                "{version}.1",
                false,
                Some("nothing before {version}".to_owned()),
            ),
            (
                "{version}.1",
                true,
                Some(format!("\".1\" after {{version}} {only}")),
            ),
            (
                "version: {version}-",
                true,
                Some(format!("\"-\" after {{version}} {only}")),
            ),
        ];
        assert_bounds(cases);
    }

    /// A digit, or a `.` and a digit, next to `{version}` lets a longer version
    /// match in part, whatever the rest of the literal holds.
    #[test]
    fn a_digit_next_to_the_version_leaves_the_side_open() {
        let near = "puts a digit, or a '.' and a digit, next to the version";
        assert_bounds([
            ("v{version}.tar", false, None),
            ("v{version}-1x", false, None),
            (
                "v{version}.5x",
                false,
                Some(format!("\".5x\" after {{version}} {near}")),
            ),
            (
                "v{version}1x",
                false,
                Some(format!("\"1x\" after {{version}} {near}")),
            ),
            (
                "x1.{version}-linux",
                false,
                Some(format!("\"x1.\" before {{version}} {near}")),
            ),
            (
                "x1{version}-linux",
                false,
                Some(format!("\"x1\" before {{version}} {near}")),
            ),
            (
                "{version}.1x",
                true,
                Some(format!("\".1x\" after {{version}} {near}")),
            ),
        ]);
    }

    /// Loads one `skill` pattern per case and checks that it loads, or that
    /// the refusal names the needle.
    fn assert_bounds<const N: usize>(cases: [(&str, bool, Option<String>); N]) {
        for (text, whole_line, refusal) in cases {
            let loaded = load(&formatdoc! {"
                {PACKAGE}
                patterns:
                  skill:
                    match: '{text}'
                    whole_line: {whole_line}

                pins:
                  - file: README.md
                    package: demo
                    patterns: [skill]
            "});
            match (loaded, refusal) {
                (Ok(_), None) => {}
                (Err(error), Some(needle)) => {
                    let report = format!("{error:#}");
                    assert!(report.contains(needle.as_str()), "{text}: {report}");
                }
                (outcome, _) => panic!("{text} (whole_line: {whole_line}): {outcome:?}"),
            }
        }
    }

    /// Every rule the schema declares, each naming what a repo has to fix.
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one case per rule; a table, not logic"
    )]
    fn a_config_that_breaks_a_rule_stops_the_load() {
        let cases = [
            (
                "an id no patterns section declares",
                formatdoc! {r#"
                    {PACKAGE}
                    patterns:
                      install:
                        match: "demo@{{version}} --"

                    pins:
                      - file: README.md
                        package: demo
                        patterns: [install, instal]
                "#},
                "pins[0].pattern_ids: no patterns.instal",
            ),
            (
                "the same id listed twice",
                formatdoc! {r#"
                    {PACKAGE}
                    patterns:
                      install:
                        match: "demo@{{version}} --"

                    pins:
                      - file: README.md
                        package: demo
                        patterns: [install, install]
                "#},
                r#"pins[0].pattern_ids: lists "install" twice"#,
            ),
            (
                "a pattern no pin lists",
                formatdoc! {r#"
                    {PACKAGE}
                    patterns:
                      install:
                        match: "demo@{{version}} --"
                      orphan:
                        match: "old@{{version}} --"

                    pins:
                      - file: README.md
                        package: demo
                        patterns: [install]
                "#},
                r#"patterns: "orphan" is declared and no pin lists it"#,
            ),
            (
                "a pin with nothing to rewrite",
                formatdoc! {"
                    {PACKAGE}
                    pins:
                      - file: README.md
                        package: demo
                "},
                "pins[0].pattern_ids: needs a tool or a pattern to rewrite",
            ),
            (
                "a spelling with nowhere to put the version",
                formatdoc! {r#"
                    {PACKAGE}
                    patterns:
                      install:
                        match: "demo@1.2.3"

                    pins:
                      - file: README.md
                        package: demo
                        patterns: [install]
                "#},
                "patterns: install: match says {version} 0 times",
            ),
            (
                "a pin reaching out of the repository",
                formatdoc! {r#"
                    {PACKAGE}
                    patterns:
                      install:
                        match: "demo@{{version}} --"

                    pins:
                      - file: ../elsewhere/README.md
                        package: demo
                        patterns: [install]
                "#},
                "pins[0].file: must stay inside the repository",
            ),
            (
                "a machine no label can find",
                formatdoc! {"
                    {PACKAGE}
                    runners:
                      ghost:
                        labels: []
                "},
                "runners.ghost.labels: must declare at least one label",
            ),
            (
                "a suffix that makes every file a template",
                formatdoc! {r#"
                    {PACKAGE}
                    templates:
                      suffix: ""
                "#},
                "templates.suffix: cannot be empty — it is what marks a file",
            ),
            (
                "a tag template naming a placeholder that does not exist",
                formatdoc! {r#"
                    {PACKAGE}
                    tags:
                      template: "v{{ver}}"
                "#},
                "tags.template: names {ver}, and the only placeholders are {version} and {name}",
            ),
            (
                "a tag template with no placeholder",
                formatdoc! {r#"
                    {PACKAGE}
                    tags:
                      template: "latest"
                "#},
                "tags.template: has no placeholder",
            ),
            (
                "a tag template with only the package name",
                formatdoc! {r#"
                    {PACKAGE}
                    tags:
                      template: "{{name}}"
                "#},
                "tags.template: must say {version}",
            ),
            (
                "an empty tag template",
                formatdoc! {r#"
                    {PACKAGE}
                    tags:
                      template: ""
                "#},
                "tags.template: cannot be empty",
            ),
            (
                "a tag template with a leftover brace",
                formatdoc! {r#"
                    {PACKAGE}
                    tags:
                      template: "v{{version}}}}"
                "#},
                "tags.template: has a '{' or '}' that is not {version} or {name}",
            ),
        ];
        for (scenario, body, expected) in cases {
            let error = load(&body).expect_err(scenario);
            assert!(
                format!("{error:#}").contains(expected),
                "{scenario}: {error:#}"
            );
        }
    }

    /// A vendored set with every required key over one declared machine and
    /// no package, plus `extra` as one more key of the set.
    fn vendored(extra: &str) -> String {
        formatdoc! {"
            runners:
              linux:
                labels: [ubuntu-24.04]
            vendored:
              lib:
                version: 1.4.0
                dir: out
                files: [lib]
                targets:
                  linux-x64:
                    runner: linux
                {extra}
        "}
    }

    #[test]
    fn a_vendored_set_alone_is_a_config_and_takes_the_default_names() {
        let config = load(&vendored("")).unwrap();
        assert!(config.packages.is_empty());
        let set = &config.vendored["lib"];
        assert_eq!(set.tag, "{name}-{version}");
        assert_eq!(set.archive, "{name}-{version}-{id}.tar.gz");
        assert_eq!(set.build, Vec::<String>::new());
        assert_eq!(set.notes, None);
    }

    #[test]
    fn a_config_with_no_package_and_no_vendored_set_fails() {
        let error = load("runners: {}\n").unwrap_err();
        assert!(
            format!("{error:#}").contains("must declare at least one package, or a vendored set"),
            "{error:#}"
        );
    }

    #[test]
    fn a_vendored_set_without_targets_fails() {
        let error = load(indoc! {"
            vendored:
              lib:
                version: 1.4.0
                dir: out
                files: [lib]
                targets: {}
        "})
        .unwrap_err();
        assert!(
            format!("{error:#}").contains("must declare at least one target"),
            "{error:#}"
        );
    }

    #[test]
    fn a_vendored_set_rejects_what_it_cannot_publish() {
        let base = vendored("");
        let cases = [
            (
                "archive without the target",
                vendored("archive: lib.tar.gz"),
                "must contain {id}",
            ),
            (
                "archive in another format",
                vendored("archive: \"{id}.zip\""),
                "must end in .tar.gz",
            ),
            (
                "archive in a directory",
                vendored("archive: \"dist/{id}.tar.gz\""),
                "must be one file name",
            ),
            (
                "archive with a typo",
                vendored("archive: \"{ids}.tar.gz\""),
                "that is not {name}, {version}, {id}",
            ),
            (
                "tag that varies by target",
                vendored("tag: \"lib-{id}\""),
                "that is not {name}, {version}",
            ),
            ("empty tag", vendored("tag: \"\""), "cannot be empty"),
            (
                "notes outside the repo",
                vendored("notes: ../notes.md"),
                "must stay inside the repository",
            ),
            ("unknown key", vendored("bin: lib"), "bin"),
            (
                "empty version",
                base.replace("version: 1.4.0", "version: \"\""),
                "cannot be empty",
            ),
            (
                "dir outside the repo",
                base.replace("dir: out", "dir: /tmp"),
                "must stay inside the repository",
            ),
            (
                "no files",
                base.replace("files: [lib]", "files: []"),
                "must declare at least one file",
            ),
            (
                "a file outside the repo",
                base.replace("files: [lib]", "files: [../secret]"),
                "must stay inside the repository",
            ),
            (
                "version with a path separator",
                base.replace("version: 1.4.0", "version: 1.4/../../x"),
                "must be one plain name",
            ),
            (
                "target key with a parent step",
                base.replace("linux-x64:", "\"..\":"),
                "must be one plain name",
            ),
            (
                "target key with a path separator",
                base.replace("linux-x64:", "a/b:"),
                "must be one plain name",
            ),
            (
                "set name with a path separator",
                base.replace("  lib:", "  \"../lib\":"),
                "must be one plain name",
            ),
            (
                "undeclared machine",
                base.replace("runner: linux", "runner: big"),
                "\"big\" is not declared in runners (declared: linux)",
            ),
        ];
        for (scenario, body, expected) in cases {
            let error = load(&body).expect_err(scenario);
            assert!(
                format!("{error:#}").contains(expected),
                "{scenario}: {error:#}"
            );
        }
    }

    /// The package config followed by `section`, an `assets` mapping.
    fn assets(section: &str) -> String {
        format!("{PACKAGE}{section}")
    }

    #[test]
    fn asset_files_are_packed_from_their_dir() {
        let config = load(&assets(indoc! {"
            assets:
              dir: target/{triple}/release
              files: [demo, demod, LICENSE]
              linux-x64: {}
        "}))
        .unwrap();
        let recipe = config.assets.unwrap();
        assert_eq!(recipe.dir, Some(PathBuf::from("target/{triple}/release")));
        assert_eq!(
            recipe.files,
            [
                PathBuf::from("demo"),
                PathBuf::from("demod"),
                PathBuf::from("LICENSE")
            ]
        );
        assert_eq!(recipe.targets.keys().collect::<Vec<_>>(), ["linux-x64"]);
    }

    #[test]
    fn a_target_lists_its_own_files() {
        let config = load(&assets(indoc! {"
            assets:
              dir: out/{triple}
              darwin-arm64:
                files: [proof]
              linux-x64:
                files: [bench, proof]
        "}))
        .unwrap();
        let recipe = config.assets.unwrap();
        assert_eq!(recipe.files, Vec::<PathBuf>::new());
        assert_eq!(
            recipe.targets["darwin-arm64"].files,
            Some(vec![PathBuf::from("proof")])
        );
        assert_eq!(
            recipe.targets["linux-x64"].files,
            Some(vec![PathBuf::from("bench"), PathBuf::from("proof")])
        );
    }

    #[test]
    fn asset_files_reject_what_they_cannot_pack() {
        let cases = [
            (
                "dir without files",
                indoc! {"
                    assets:
                      dir: out
                      linux-x64: {}
                "},
                "must come with files",
            ),
            (
                "dir with no files",
                indoc! {"
                    assets:
                      dir: out
                      files: []
                      linux-x64: {}
                "},
                "must come with files",
            ),
            (
                "files without dir",
                indoc! {"
                    assets:
                      files: [demo]
                      linux-x64: {}
                "},
                "must come with dir",
            ),
            (
                "binary beside files",
                indoc! {"
                    assets:
                      binary: out/{bin}
                      dir: out
                      files: [demo]
                      linux-x64: {}
                "},
                "list the binary in files",
            ),
            (
                "dir outside the repo",
                indoc! {"
                    assets:
                      dir: /tmp
                      files: [demo]
                      linux-x64: {}
                "},
                "must stay inside the repository",
            ),
            (
                "a file outside the repo",
                indoc! {"
                    assets:
                      dir: out
                      files: [../secret]
                      linux-x64: {}
                "},
                "must stay inside the repository",
            ),
        ];
        for (scenario, section, expected) in cases {
            let error = load(&assets(section)).expect_err(scenario);
            assert!(
                format!("{error:#}").contains(expected),
                "{scenario}: {error:#}"
            );
        }
    }

    #[test]
    fn asset_target_files_reject_what_they_cannot_pack() {
        let cases = [
            (
                "target files without dir",
                indoc! {"
                    assets:
                      linux-x64:
                        files: [demo]
                "},
                "assets.linux-x64.files: must come with assets.dir",
            ),
            (
                "an empty target list",
                indoc! {"
                    assets:
                      dir: out
                      files: [demo]
                      linux-x64:
                        files: []
                "},
                "assets.linux-x64.files: must declare at least one file",
            ),
            (
                "binary beside target files",
                indoc! {"
                    assets:
                      binary: out/{bin}
                      darwin-arm64: {}
                      linux-x64:
                        files: [demo]
                "},
                "assets.binary: must be omitted when assets.linux-x64.files says what the tarball holds",
            ),
            (
                "a target file outside the repo",
                indoc! {"
                    assets:
                      dir: out
                      linux-x64:
                        files: [../secret]
                "},
                "assets.linux-x64.files[0]: must stay inside the repository",
            ),
            (
                "dir with a target that lists no files",
                indoc! {"
                    assets:
                      dir: out
                      darwin-arm64: {}
                      linux-x64:
                        files: [demo]
                "},
                "must come with files",
            ),
        ];
        for (scenario, section, expected) in cases {
            let error = load(&assets(section)).expect_err(scenario);
            assert!(
                format!("{error:#}").contains(expected),
                "{scenario}: {error:#}"
            );
        }
    }

    /// The targets map is flattened under `assets`, so the file has no
    /// `targets` key. The problem names the path the file holds and sits on
    /// the line of `files`, the sixth after the three package lines.
    #[test]
    fn an_asset_target_problem_sits_on_its_own_line() {
        let error = load(&assets(indoc! {"
            assets:
              linux-x64:
                files: [demo]
        "}))
        .expect_err("target files without dir");
        assert!(
            format!("{error:#}")
                .contains(".ctl/ver.yaml:6:12: assets.linux-x64.files: must come with assets.dir"),
            "{error:#}"
        );
    }

    const ONE_PACKAGE: &str = indoc! {"
        packages:
          - name: demo
            path: Cargo.toml
    "};

    #[test]
    fn a_proposal_left_out_is_todays_title_and_message() {
        let config = load(ONE_PACKAGE).unwrap();
        assert_eq!(config.proposal.title(), "chore(release): version packages");
        assert_eq!(
            config.proposal.commit_message(),
            "chore(release): version packages"
        );
    }

    #[test]
    fn a_title_alone_is_also_the_commit_message() {
        let config = load(&formatdoc! {"
            {ONE_PACKAGE}proposal:
              title: 'chore: release'
        "})
        .unwrap();
        assert_eq!(config.proposal.title(), "chore: release");
        assert_eq!(config.proposal.commit_message(), "chore: release");
    }

    #[test]
    fn an_empty_title_and_message_are_unset() {
        let config = load(&formatdoc! {"
            {ONE_PACKAGE}proposal:
              title: ''
              commit_message: ''
        "})
        .unwrap();
        assert_eq!(config.proposal.title(), "chore(release): version packages");
        assert_eq!(
            config.proposal.commit_message(),
            "chore(release): version packages"
        );
    }

    #[test]
    fn a_commit_message_differs_from_the_title() {
        let config = load(&formatdoc! {"
            {ONE_PACKAGE}proposal:
              title: 'chore: release'
              commit_message: 'chore: release [skip ci]'
        "})
        .unwrap();
        assert_eq!(config.proposal.title(), "chore: release");
        assert_eq!(config.proposal.commit_message(), "chore: release [skip ci]");
    }

    #[test]
    fn a_version_label_is_refused_with_what_to_do() {
        for value in [" release", "", " ~", " null"] {
            let error = load(&formatdoc! {"
                {ONE_PACKAGE}prepare:
                  version_label:{value}
            "})
            .unwrap_err();
            let error = format!("{error:#}");
            assert!(
                error.contains("prepare.version_label is removed"),
                "{value:?}: {error}"
            );
            assert!(error.contains("no longer labeled"), "{value:?}: {error}");
            assert!(error.contains("delete this key"), "{value:?}: {error}");
        }
    }

    /// A misspelled key under a package, whether its own or a flattened
    /// driver field, fails the load and names the key and the package.
    #[test]
    fn an_unknown_key_under_a_package_fails() {
        for (line, key) in [
            ("publsher: none", "publsher"),
            ("foramt: toml", "foramt"),
            ("keys: [package.version]\n    kes: [x]", "kes"),
        ] {
            let error = load(&formatdoc! {"
                packages:
                  - name: demo
                    path: Cargo.toml
                    {line}
            "})
            .unwrap_err();
            let error = format!("{error:#}");
            assert!(error.contains(key), "{line}: {error}");
            assert!(error.contains("\"demo\""), "{line}: {error}");
        }
        load(indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
                format: toml
                keys: [package.version]
                publisher: none
        "})
        .unwrap();
    }

    #[test]
    fn an_unknown_key_under_a_pin_fails() {
        let error = load(&formatdoc! {"
            {ONE_PACKAGE}pins:
              - file: README.md
                package: demo
                tool: github:acme/demo
                pattern: [readme]
        "})
        .unwrap_err();
        assert!(format!("{error:#}").contains("pattern"), "{error:#}");
    }

    #[test]
    fn an_unknown_key_under_a_pattern_fails() {
        let error = load(&formatdoc! {"
            {ONE_PACKAGE}patterns:
              readme:
                match: 'demo@{{version}} in CI'
                ocurrences: many
            pins:
              - file: README.md
                package: demo
                patterns: [readme]
        "})
        .unwrap_err();
        assert!(format!("{error:#}").contains("ocurrences"), "{error:#}");
    }

    #[test]
    fn an_unknown_key_under_prepare_fails() {
        let error = load(&formatdoc! {"
            {ONE_PACKAGE}prepare:
              stages: [docs]
        "})
        .unwrap_err();
        assert!(format!("{error:#}").contains("stages"), "{error:#}");
    }

    #[test]
    fn an_unknown_key_under_proposal_fails() {
        let error = load(&formatdoc! {"
            {ONE_PACKAGE}proposal:
              label: release
        "})
        .unwrap_err();
        assert!(format!("{error:#}").contains("label"), "{error:#}");
    }

    #[test]
    fn publish_reads_options_or_a_whole_command() {
        let config = load(indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
                publish:
                  verify: false
              - name: tool
                path: tool/pyproject.toml
                publish: [uv, publish]
              - name: quiet
                path: quiet/Cargo.toml
                publish: []
        "})
        .unwrap();
        let options = config.packages[0].publish_options().unwrap();
        assert_eq!(options.get("verify"), Some(&serde_json::Value::Bool(false)));
        assert_eq!(config.packages[0].publish_command(), None);
        assert_eq!(
            config.packages[1].publish_command(),
            Some(["uv".to_owned(), "publish".to_owned()].as_slice())
        );
        assert_eq!(config.packages[2].publish_command(), Some([].as_slice()));
    }

    #[test]
    fn publish_of_another_shape_names_both() {
        let error = load(indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
                publish: fast
        "})
        .unwrap_err();
        assert!(
            format!("{error:#}").contains(
                "a mapping of publisher options, or a list that is the whole upload command"
            ),
            "{error:#}"
        );
    }

    /// Each option belongs to the publisher the package resolves to from the
    /// file alone, and the load names what to remove.
    #[test]
    fn options_are_checked_against_the_publisher_at_load() {
        for (yaml, message) in [
            (
                indoc! {"
                    packages:
                      - name: web
                        path: package.json
                        publish:
                          verify: false
                "},
                r#"package "web" publishes with bun, which has no publish options; remove publish.verify"#,
            ),
            (
                indoc! {"
                    packages:
                      - name: demo
                        path: Cargo.toml
                        publish:
                          allow_dirty: true
                "},
                r#"package "demo": cargo has no publish option "allow_dirty"; its options are: verify"#,
            ),
            (
                indoc! {"
                    packages:
                      - name: demo
                        path: Cargo.toml
                        publish:
                          verify: no thanks
                "},
                r#"package "demo": publish.verify is true or false"#,
            ),
            (
                indoc! {"
                    publishers:
                      cargo:
                        argv: [cargo, publish, --workspace]
                    packages:
                      - name: demo
                        path: Cargo.toml
                        publish:
                          verify: false
                "},
                r#"package "demo": publishers.cargo replaces the upload command, which takes no options; remove publish, or the argv"#,
            ),
        ] {
            let error = load(yaml).unwrap_err();
            assert!(format!("{error:#}").contains(message), "{error:#}");
        }
    }
}
