use crate::driver::{CommandSpec, Driver, Format};
use crate::publisher::PublisherSpec;
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum CommandField {
    Mise(String),
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

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DriverSpec {
    pub format: Option<String>,
    pub keys: Option<Vec<String>>,
    pub read: Option<CommandField>,
    pub write: Option<CommandField>,
    pub after: Option<String>,
}

impl DriverSpec {
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

#[derive(Debug, Clone, Deserialize)]
pub struct PackageSpec {
    pub name: String,
    pub path: PathBuf,
    pub driver: Option<String>,
    /// Stock `cargo` / `bun`, a `[publishers.NAME]` key, or omit to infer.
    pub publisher: Option<String>,
    /// Pretty noun (`crate`, `package`, `wheel`). Defaults from the publisher.
    pub noun: Option<String>,
    /// Override the publisher argv for this package.
    #[serde(default, rename = "publish")]
    pub publish_argv: Option<Vec<String>>,
    pub registry: Option<String>,
    /// Package CHANGELOG.md. Defaults to next to the manifest.
    pub changelog: Option<PathBuf>,
    #[serde(flatten)]
    pub spec: DriverSpec,
}

impl PackageSpec {
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

    pub fn resolve(&self, config: &Config, root: &Path) -> Result<Driver> {
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
}

/// One machine, declared once as `[runners.NAME]`. The header names it; this
/// is how GitHub finds it. Every label is required at once, so a three-label
/// runner is one machine carrying three labels, not three machines.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Runner {
    pub labels: Vec<String>,
}

/// One job, declared as `[ci.NAME]`. The header names it; `runners` names the
/// machines from `[runners]` it runs on, one check each. No `os`/`arch`/
/// `triple`: validation runs a machine's checks, it does not cross-compile.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Job {
    pub runners: Option<Vec<String>>,
}

/// Native GitHub Release tarballs, plus the recipe every target shares. Omit
/// for libraries. Each `[assets.NAME]` sub-table is one target.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Assets {
    pub bin: Option<String>,
    /// Ran once per target before `build`. Stock rust recipe uses rustup.
    pub prepare: Option<Vec<String>>,
    /// How to produce the binary. Stock rust recipe is `cargo build --release`.
    pub build: Option<Vec<String>>,
    /// Path to the built binary, with `{bin}` `{triple}` `{os}` `{arch}`.
    pub binary: Option<String>,
    /// Retired `targets = [...]` list, captured only so the repos still on it
    /// get the migration instead of a serde type error from the map below.
    /// Remove once qctl and ctl-core have bumped their verctl pin.
    #[serde(default, rename = "targets")]
    pub retired_targets: Option<toml::Value>,
    /// Every remaining sub-table: `[assets.linux-x64]` and friends. A typo'd
    /// recipe key lands here and fails as an unknown target, loudly.
    #[serde(flatten)]
    pub targets: BTreeMap<String, AssetTarget>,
}

/// One build target: the platform it produces and the single machine that
/// builds it. One tarball, one machine — so `runner` is not a list.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct AssetTarget {
    pub runner: Option<String>,
    pub os: Option<String>,
    pub arch: Option<String>,
    pub triple: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub drivers: BTreeMap<String, DriverSpec>,
    pub packages: Vec<PackageSpec>,
    #[serde(default)]
    pub publishers: BTreeMap<String, PublisherSpec>,
    /// Machines, declared once and named. Entirely repo-owned: verctl ships no
    /// built-in names.
    #[serde(default)]
    pub runners: BTreeMap<String, Runner>,
    #[serde(default)]
    pub assets: Option<Assets>,
    /// PR and push validation. Empty means one `verify` on `ubuntu-latest`.
    #[serde(default)]
    pub ci: BTreeMap<String, Job>,
    #[serde(default)]
    pub prepare: Prepare,
    /// Collocated tool pins rewritten when `package` is bumped.
    #[serde(default)]
    pub pins: Vec<Pin>,
}

/// A mise `[tools]` entry that must track a package version.
#[derive(Debug, Clone, Deserialize)]
pub struct Pin {
    pub file: PathBuf,
    pub tool: String,
    pub package: String,
}

/// Extra work after version bumps, committed on the Version PR.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Prepare {
    /// argv after all bumps (e.g. `["mise", "run", "version-sync"]`).
    pub after: Option<Vec<String>>,
    /// Extra paths `after` may write. Anything else dirty fails.
    #[serde(default)]
    pub stage: Vec<String>,
    /// Collect gitignored paths that match `stage`. Off by default.
    #[serde(default)]
    pub stage_ignored: bool,
    /// Label `prepare --pr` applies. `check --versions` trusts this
    /// on the GitHub event, not `GITHUB_HEAD_REF`.
    pub version_label: Option<String>,
}

impl Prepare {
    pub const DEFAULT_VERSION_LABEL: &'static str = "verctl:version";

    #[must_use]
    pub fn version_label(&self) -> &str {
        self.version_label
            .as_deref()
            .filter(|label| !label.is_empty())
            .unwrap_or(Self::DEFAULT_VERSION_LABEL)
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path).with_context(|| path.display().to_string())?;
        let config: Self =
            toml::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
        if config.packages.is_empty() {
            bail!("{} has no [[packages]]", path.display());
        }
        Ok(config)
    }

    pub fn find(&self, name: &str) -> Result<&PackageSpec> {
        self.packages
            .iter()
            .find(|package| package.name == name)
            .with_context(|| format!("package {name:?} is not in verctl config"))
    }

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
        _ => None,
    }
}

pub(crate) fn stock_driver(name: &str) -> Result<Driver> {
    DriverSpec::stock(name)
        .with_context(|| format!("unknown stock driver {name:?}"))?
        .into_driver(name)
}
