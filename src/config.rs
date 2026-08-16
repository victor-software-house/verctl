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

#[derive(Debug, Clone, Deserialize)]
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

    fn is_declared(&self) -> bool {
        self.format.is_some() || self.keys.is_some() || self.read.is_some() || self.write.is_some()
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
    pub fn resolve(&self, config: &Config) -> Result<Driver> {
        if self.spec.is_declared() {
            return self.spec.clone().into_driver(&self.name);
        }
        if let Some(name) = &self.driver {
            return config.driver(name);
        }
        infer_driver(&self.path)
    }
}

fn infer_driver(path: &Path) -> Result<Driver> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("invalid path {}", path.display()))?;
    let name = crate::generated::infer_name(file_name).with_context(|| {
        format!("cannot infer driver from {file_name} (set driver, format+keys, or read+write)")
    })?;
    crate::generated::stock(name)
        .with_context(|| format!("stock driver {name:?} missing from generated table"))
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

    pub fn driver(&self, name: &str) -> Result<Driver> {
        if let Some(spec) = self.drivers.get(name) {
            return spec.clone().into_driver(name);
        }
        crate::generated::stock(name).with_context(|| format!("unknown driver {name:?}"))
    }
}
