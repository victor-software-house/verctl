use crate::driver::{CommandSpec, Driver, Format};
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
    /// Where `publish` ships this package.
    ///
    /// Cargo: omit or `crates-io`. Any other name is `cargo publish --registry`.
    /// Bun: omit or `npm` is registry.npmjs.org (`--access public`).
    /// `github` is `bun publish --registry https://npm.pkg.github.com`.
    /// A URL is passed through as `--registry`. Always
    /// `--tolerate-republish`.
    pub registry: Option<String>,
    #[serde(flatten)]
    pub spec: DriverSpec,
}

impl PackageSpec {
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

/// Native GitHub Release tarballs. Omit for libraries, or list one
/// target when a single binary is enough.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Assets {
    pub bin: Option<String>,
    #[serde(default)]
    pub targets: Vec<AssetTarget>,
}

/// `"linux-x64"` or `{ id = "linux-x64", runner = "ubuntu-24.04" }`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum AssetTarget {
    Id(String),
    Spec { id: String, runner: Option<String> },
}

impl AssetTarget {
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Id(id) | Self::Spec { id, .. } => id,
        }
    }

    #[must_use]
    pub fn runner(&self) -> Option<&str> {
        match self {
            Self::Id(_) => None,
            Self::Spec { runner, .. } => runner.as_deref(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub drivers: BTreeMap<String, DriverSpec>,
    pub packages: Vec<PackageSpec>,
    #[serde(default)]
    pub assets: Option<Assets>,
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
