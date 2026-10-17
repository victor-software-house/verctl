//! How a package is published. Cargo and bun are stock recipes.
//!
//! Override a `publishers` entry or set `publisher` + `argv` on a
//! package. Placeholders: `{path}` `{dir}` `{registry}` `{config}`, and in a
//! `query` also `{name}` `{version}`. [`RELEASE_ONLY`] publishes to no
//! registry.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;

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
    /// directory, where the publisher's registry config is in effect. Unsaid,
    /// the stock recipe's query applies; an empty list turns the check off,
    /// leaving the stderr match.
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

    /// Render the label and argv for one package, expanding placeholders.
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
        let out = self.assemble(argv, manifest, registry, &[], Appended::Upload);
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
    ) -> Vec<String> {
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
        let mut out = expand(argv, &ctx);
        let (recipe, default) = match appended {
            Appended::Upload => (true, true),
            Appended::StockQuery => (true, false),
            Appended::Nothing => (false, false),
        };
        if recipe
            && config.is_some()
            && let Some(extra) = &self.config_argv
        {
            out.extend(expand(extra, &ctx));
        }
        if recipe && config.is_none() && registry.is_some() {
            if let Some(extra) = &self.registry_argv {
                out.extend(expand(extra, &ctx));
            }
        } else if default
            && registry.is_none()
            && let Some(extra) = &self.default_argv
        {
            out.extend(expand(extra, &ctx));
        }
        out
    }
}

/// What [`PublisherSpec::assemble`] appends after the expanded argv.
#[derive(Debug, Clone, Copy)]
enum Appended {
    /// The recipe's config or registry arguments, else `default_argv`
    /// (`--access public`): the full upload command.
    Upload,
    /// The recipe's config or registry arguments, so a stock query asks the
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

/// The stock bun query. `bun pm view` is an alias of `bun info`, which prints
/// one property of the selected version and fails when the version does not
/// exist: <https://bun.com/docs/pm/cli/info>.
const BUN_QUERY: [&str; 5] = ["bun", "pm", "view", "{name}@{version}", "version"];

/// The publisher name for a package released only on GitHub: a tag and a
/// Release, with assets when declared, and no registry step. Its argv is
/// empty, which `publish` reads as "nothing to upload"; every other publisher
/// must render a command, so an empty one never passes for this mode.
pub const RELEASE_ONLY: &str = "none";

/// Resolve a package's noun, label, and upload command from config and stock
/// recipes.
pub fn resolve(
    config: &crate::config::Config,
    spec: &crate::config::PackageSpec,
    manifest: &Path,
) -> Result<(String, String, Vec<String>)> {
    if spec.publisher.as_deref() == Some(RELEASE_ONLY) {
        let noun = spec.noun.clone().unwrap_or_else(|| "package".into());
        return Ok(("release only".into(), noun, Vec::new()));
    }
    let name = publisher_name(spec);
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
    ensure!(
        !argv.is_empty(),
        "package {:?}: publisher {name:?} renders an empty command; set publisher: none to release without a registry",
        spec.name
    );
    Ok((name.to_owned(), noun, argv))
}

/// The publisher a package names, or the stock one its manifest file implies,
/// or a `publishers` entry under the package's own name.
fn publisher_name(spec: &crate::config::PackageSpec) -> &str {
    let inferred = spec
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(stock_publisher_for_file);
    spec.publisher
        .as_deref()
        .or(inferred)
        .unwrap_or(spec.name.as_str())
}

/// How `publish` decides whether `version` of a package is already on its
/// registry.
///
/// A package's own `query` wins, then its `publishers` entry's, and an empty
/// one turns the check off. Otherwise a stock cargo package whose publish
/// target is crates.io is read from the sparse index, and a stock recipe's
/// query applies. A cargo package with an overridden command, or with a target
/// verctl cannot tell, has no check unless it declares `query`: verctl cannot
/// know the directory such a command runs in, nor the registry it uploads to.
///
/// The registry is asked for the name the manifest declares (`package.name`
/// in `Cargo.toml`, `name` in `package.json`), not the label `.ctl/ver.yaml`
/// gives the package. A stock query takes the recipe's registry or config
/// arguments, so it asks the registry the upload goes to; a declared query
/// runs as written, with `{registry}` and `{config}` to place them itself.
///
/// # Errors
///
/// Returns an error when the manifest cannot be read or parsed, or names no
/// package.
pub fn registry_check(
    config: &crate::config::Config,
    spec: &crate::config::PackageSpec,
    root: &Path,
    version: &str,
) -> Result<RegistryCheck> {
    if spec.publisher.as_deref() == Some(RELEASE_ONLY) {
        return Ok(RegistryCheck::Unchecked);
    }
    let name = publisher_name(spec);
    let over = config.publishers.get(name);
    let publisher = match (PublisherSpec::stock(name), over) {
        (Some(stock), Some(over)) => stock.merge(over.clone()),
        (Some(stock), None) => stock,
        (None, Some(over)) => over.clone(),
        (None, None) => PublisherSpec::default(),
    };
    let manifest = root.join(&spec.path);
    let declared = spec
        .query
        .as_ref()
        .or_else(|| over.and_then(|over| over.query.as_ref()));
    if let Some(argv) = declared {
        return query_check(
            &publisher,
            argv,
            spec,
            &manifest,
            version,
            Appended::Nothing,
        );
    }
    let overridden = spec.publish_argv.is_some() || over.is_some_and(|over| over.argv.is_some());
    if name == "cargo" && !overridden {
        let target = registry::cargo_target(
            spec.registry.as_deref(),
            &manifest,
            root,
            &CargoEnv::from_env(),
        )?;
        return Ok(match target {
            CargoTarget::CratesIo => RegistryCheck::CratesIo {
                name: manifest_name(&manifest)?,
            },
            CargoTarget::Other | CargoTarget::Unknown => RegistryCheck::Unchecked,
        });
    }
    match publisher.query.clone() {
        Some(argv) => query_check(
            &publisher,
            &argv,
            spec,
            &manifest,
            version,
            Appended::StockQuery,
        ),
        None => Ok(RegistryCheck::Unchecked),
    }
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
        argv: publisher.assemble(argv, manifest, spec.registry.as_deref(), &extra, appended),
        dir: package_dir(manifest).to_path_buf(),
    })
}

/// The package name a manifest declares: `package.name` in `Cargo.toml`,
/// `name` in `package.json`, which is what the registry knows it by.
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
    use std::path::Path;

    use indoc::indoc;

    use super::{PublisherSpec, registry_check, resolve};
    use crate::registry::RegistryCheck;

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
        let manifest = Path::new("Cargo.toml");
        let (label, _, argv) = resolve(&config, &config.packages[0], manifest).unwrap();
        assert_eq!(label, "release only");
        assert_eq!(argv, Vec::<String>::new());
        let error = resolve(&config, &config.packages[1], manifest).unwrap_err();
        assert!(
            format!("{error:#}").contains("renders an empty command"),
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
