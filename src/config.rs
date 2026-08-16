use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ManifestKind {
    Cargo,
    Npm,
}

impl ManifestKind {
    fn infer(path: &Path) -> Result<Self> {
        match path.file_name().and_then(|name| name.to_str()) {
            Some("Cargo.toml") => Ok(Self::Cargo),
            Some("package.json") => Ok(Self::Npm),
            _ => bail!(
                "cannot infer manifest kind from {} (set kind = \"cargo\" or \"npm\")",
                path.display()
            ),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PackageSpec {
    pub name: String,
    pub path: PathBuf,
    pub kind: Option<ManifestKind>,
}

impl PackageSpec {
    pub fn kind(&self) -> Result<ManifestKind> {
        self.kind
            .map_or_else(|| ManifestKind::infer(&self.path), Ok)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
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
}
