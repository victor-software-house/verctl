//! How a package is published. Cargo and bun are stock recipes.
//!
//! Override a `publishers` entry or set `publisher` + `argv` on a
//! package. Placeholders: `{path}` `{dir}` `{registry}` `{config}`.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PublisherSpec {
    pub noun: Option<String>,
    pub argv: Option<Vec<String>>,
    /// Ancestor file that owns registry/auth (`bunfig.toml`).
    pub config_name: Option<String>,
    pub config_argv: Option<Vec<String>>,
    /// Appended when `registry` is set and does not alias to empty.
    pub registry_argv: Option<Vec<String>>,
    /// Appended when there is no registry (or the alias is empty).
    pub default_argv: Option<Vec<String>>,
    #[serde(default)]
    pub registries: BTreeMap<String, String>,
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
        }
    }

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
        let dir = manifest
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let config = self
            .config_name
            .as_deref()
            .and_then(|file| find_config(manifest, file));
        let registry = resolve_registry(&self.registries, registry);
        let mut ctx = BTreeMap::from([
            ("path", manifest.display().to_string()),
            ("dir", dir.display().to_string()),
        ]);
        if let Some(path) = &config {
            ctx.insert("config", path.display().to_string());
        }
        if let Some(registry) = &registry {
            ctx.insert("registry", registry.clone());
        }
        let mut out = expand(argv, &ctx);
        if config.is_some()
            && let Some(extra) = &self.config_argv
        {
            out.extend(expand(extra, &ctx));
        }
        if config.is_none() && registry.is_some() {
            if let Some(extra) = &self.registry_argv {
                out.extend(expand(extra, &ctx));
            }
        } else if registry.is_none()
            && let Some(extra) = &self.default_argv
        {
            out.extend(expand(extra, &ctx));
        }
        let noun = self.noun.clone().unwrap_or_else(|| "package".into());
        Ok((noun, out))
    }
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

fn expand(parts: &[String], ctx: &BTreeMap<&str, String>) -> Vec<String> {
    parts
        .iter()
        .map(|part| {
            let mut out = part.clone();
            for (key, value) in ctx {
                out = out.replace(&format!("{{{key}}}"), value);
            }
            out
        })
        .filter(|part| !part.is_empty() && !part.contains('{'))
        .collect()
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

pub fn resolve(
    config: &crate::config::Config,
    spec: &crate::config::PackageSpec,
    manifest: &Path,
) -> Result<(String, String, Vec<String>)> {
    let inferred = spec
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(stock_publisher_for_file);
    let name = spec
        .publisher
        .as_deref()
        .or(inferred)
        .unwrap_or(spec.name.as_str());
    let mut publisher = match (PublisherSpec::stock(name), config.publishers.get(name)) {
        (Some(stock), Some(over)) => stock.merge(over.clone()),
        (Some(stock), None) => stock,
        (None, Some(over)) => over.clone(),
        (None, None) => {
            if let Some(argv) = &spec.publish_argv {
                PublisherSpec {
                    noun: spec.noun.clone(),
                    argv: Some(argv.clone()),
                    ..PublisherSpec::default()
                }
            } else {
                bail!(
                    "package {:?} has no publisher (set publisher / publishers.{name} / argv)",
                    spec.name
                );
            }
        }
    };
    if spec.noun.is_some() {
        publisher.noun.clone_from(&spec.noun);
    }
    if spec.publish_argv.is_some() {
        publisher.argv.clone_from(&spec.publish_argv);
    }
    let (noun, argv) = publisher.render(name, manifest, spec.registry.as_deref())?;
    Ok((name.to_owned(), noun, argv))
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
    use super::PublisherSpec;
    use std::path::Path;

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
}
