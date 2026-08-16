use crate::driver::{CommandSpec, Driver, Format};
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

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
    #[serde(flatten)]
    pub spec: DriverSpec,
}

impl PackageSpec {
    pub fn resolve(&self, config: &Config, root: &Path) -> Result<Driver> {
        let inferred = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(infer_stock_name);
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

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub drivers: BTreeMap<String, DriverSpec>,
    pub packages: Vec<PackageSpec>,
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
        let stock = stock_file().drivers.get(name).cloned();
        match (stock, self.drivers.get(name)) {
            (Some(stock), Some(over)) => Some(stock.merge(over.clone())),
            (Some(stock), None) => Some(stock),
            (None, Some(over)) => Some(over.clone()),
            (None, None) => None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct StockFile {
    drivers: BTreeMap<String, DriverSpec>,
    filenames: BTreeMap<String, String>,
}

fn stock_file() -> &'static StockFile {
    static STOCK: OnceLock<StockFile> = OnceLock::new();
    STOCK.get_or_init(|| {
        toml::from_str(include_str!("../drivers.toml"))
            .unwrap_or_else(|error| panic!("drivers.toml is invalid: {error}"))
    })
}

fn infer_stock_name(file_name: &str) -> Option<&str> {
    stock_file().filenames.get(file_name).map(String::as_str)
}

pub(crate) fn stock_driver(name: &str) -> Result<Driver> {
    stock_file()
        .drivers
        .get(name)
        .cloned()
        .with_context(|| format!("unknown stock driver {name:?}"))?
        .into_driver(name)
}
