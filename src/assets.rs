//! Native release assets. Only the targets listed in `[assets]`.
//!
//! Omit `[assets]` when there is no host binary. Override `prepare`,
//! `build`, and `binary` when the stock rust recipe is the wrong stack.
//! PR CI never reads this list.

use crate::config::Config;
use crate::github;
use crate::process;
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const BUILD_TIMEOUT: Duration = Duration::from_mins(15);

/// One row of a GitHub Actions `strategy.matrix`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MatrixTarget {
    pub id: String,
    pub runner: String,
    pub os: String,
    pub arch: String,
    pub triple: String,
    pub asset: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Matrix {
    pub include: Vec<MatrixTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssetsPlan {
    pub bin: String,
    pub version: String,
    pub tag: String,
    pub has_assets: bool,
    pub matrix: Matrix,
    #[serde(skip)]
    prepare: Vec<String>,
    #[serde(skip)]
    build: Vec<String>,
    #[serde(skip)]
    binary: String,
}

#[derive(Clone, Copy, Debug)]
struct Known {
    id: &'static str,
    runner: &'static str,
    os: &'static str,
    arch: &'static str,
    triple: &'static str,
}

const KNOWN: &[Known] = &[
    Known {
        id: "darwin-arm64",
        runner: "macos-latest",
        os: "darwin",
        arch: "arm64",
        triple: "aarch64-apple-darwin",
    },
    Known {
        id: "linux-x64",
        runner: "ubuntu-latest",
        os: "linux",
        arch: "x64",
        triple: "x86_64-unknown-linux-gnu",
    },
];

const STOCK_PREPARE: &[&str] = &["rustup", "target", "add", "{triple}"];
const STOCK_BUILD: &[&str] = &[
    "cargo",
    "build",
    "--release",
    "--locked",
    "--target",
    "{triple}",
];
const STOCK_BINARY: &str = "target/{triple}/release/{bin}";

pub fn plan(config: &Config, root: &Path) -> Result<AssetsPlan> {
    let package = config
        .packages
        .first()
        .context("verctl.toml has no [[packages]]")?;
    let driver = package.resolve(config, root)?;
    let raw = fs::read_to_string(root.join(&package.path))
        .with_context(|| package.path.display().to_string())?;
    let version = driver.read(&raw)?.trim().to_owned();
    let bin = config
        .assets
        .as_ref()
        .and_then(|assets| assets.bin.clone())
        .unwrap_or_else(|| package.name.clone());
    let ids = config
        .assets
        .as_ref()
        .map_or(&[][..], |assets| assets.targets.as_slice());
    let assets = config.assets.clone().unwrap_or_default();
    let custom_build = assets.build.is_some();
    let mut include = Vec::new();
    for spec in ids {
        include.push(resolve_target(spec, &bin, &version)?);
    }
    Ok(AssetsPlan {
        bin,
        version: version.clone(),
        tag: format!("v{version}"),
        has_assets: !include.is_empty(),
        matrix: Matrix { include },
        prepare: assets.prepare.unwrap_or_else(|| {
            if custom_build {
                Vec::new()
            } else {
                strings(STOCK_PREPARE)
            }
        }),
        build: assets.build.unwrap_or_else(|| strings(STOCK_BUILD)),
        binary: assets.binary.unwrap_or_else(|| STOCK_BINARY.to_owned()),
    })
}

pub fn write_github_output(plan: &AssetsPlan, path: &Path) -> Result<()> {
    let matrix = serde_json::to_string(&plan.matrix).context("encode matrix")?;
    let body = formatdoc_output(plan, &matrix);
    fs::write(path, body).with_context(|| path.display().to_string())
}

fn formatdoc_output(plan: &AssetsPlan, matrix: &str) -> String {
    use ctl_core::formatdoc;
    formatdoc! {"
        has_assets={}
        tag={}
        matrix={}
        ",
        plan.has_assets,
        plan.tag,
        matrix,
    }
}

pub fn build(plan: &AssetsPlan, id: &str, root: &Path) -> Result<PathBuf> {
    let target = plan
        .matrix
        .include
        .iter()
        .find(|row| row.id == id)
        .with_context(|| format!("target {id:?} is not in [assets].targets"))?;
    let ctx = target_ctx(plan, target);
    if !plan.prepare.is_empty() {
        let prepare = expand(&plan.prepare, &ctx);
        process::run_limited(&prepare, &[], BUILD_TIMEOUT).context("assets.prepare")?;
    }
    let build = expand(&plan.build, &ctx);
    process::run_limited(&build, &[], BUILD_TIMEOUT).context("assets.build")?;
    let binary = root.join(expand_one(&plan.binary, &ctx));
    ensure!(
        binary.is_file(),
        "missing release binary {}",
        binary.display()
    );
    let tarball = root.join(&target.asset);
    let tar = process::argv(&[
        "tar",
        "czf",
        tarball.to_str().context("tarball path")?,
        "-C",
        binary
            .parent()
            .context("binary parent")?
            .to_str()
            .context("dir")?,
        &plan.bin,
    ]);
    process::run_limited(&tar, &[], Duration::from_mins(2)).context("tar")?;
    Ok(tarball)
}

pub fn upload(root: &Path, tag: &str, tarball: &Path) -> Result<String> {
    let token = crate::release::resolve_token()?;
    let repo = github::repo(root)?;
    github::upload_release_asset(&token, &repo, tag, tarball)
}

fn resolve_target(
    spec: &crate::config::AssetTarget,
    bin: &str,
    version: &str,
) -> Result<MatrixTarget> {
    let known = KNOWN.iter().find(|known| known.id == spec.id());
    let runner = spec
        .runner()
        .or_else(|| known.map(|known| known.runner))
        .with_context(|| {
            format!(
                "unknown asset target {:?} needs runner= (or use {})",
                spec.id(),
                KNOWN
                    .iter()
                    .map(|known| known.id)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;
    let os = spec
        .os()
        .or_else(|| known.map(|known| known.os))
        .unwrap_or("unknown");
    let arch = spec
        .arch()
        .or_else(|| known.map(|known| known.arch))
        .unwrap_or("unknown");
    let triple = spec
        .triple()
        .or_else(|| known.map(|known| known.triple))
        .unwrap_or("");
    let asset_os = if os == "darwin" { "macos" } else { os };
    Ok(MatrixTarget {
        id: spec.id().to_owned(),
        runner: runner.to_owned(),
        os: os.to_owned(),
        arch: arch.to_owned(),
        triple: triple.to_owned(),
        asset: format!("{bin}_{version}_{asset_os}_{arch}.tar.gz"),
    })
}

fn strings(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
}

fn target_ctx(plan: &AssetsPlan, target: &MatrixTarget) -> BTreeMap<&'static str, String> {
    BTreeMap::from([
        ("bin", plan.bin.clone()),
        ("version", plan.version.clone()),
        ("id", target.id.clone()),
        ("runner", target.runner.clone()),
        ("os", target.os.clone()),
        ("arch", target.arch.clone()),
        ("triple", target.triple.clone()),
    ])
}

fn expand(parts: &[String], ctx: &BTreeMap<&str, String>) -> Vec<String> {
    parts.iter().map(|part| expand_one(part, ctx)).collect()
}

fn expand_one(part: &str, ctx: &BTreeMap<&str, String>) -> String {
    let mut out = part.to_owned();
    for (key, value) in ctx {
        out = out.replace(&format!("{{{key}}}"), value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::plan;
    use crate::config::Config;
    use anyhow::Result;

    fn load(toml: &str) -> Result<Config> {
        toml::from_str(toml).map_err(Into::into)
    }

    fn write_cargo(root: &std::path::Path, version: &str) {
        std::fs::write(
            root.join("Cargo.toml"),
            indoc::formatdoc! {r#"
                [package]
                version = "{version}"
            "#},
        )
        .unwrap();
    }

    #[test]
    fn omitted_assets_means_no_native_jobs() {
        let config = load(indoc::indoc! {r#"
            [[packages]]
            name = "ctl-core"
            path = "Cargo.toml"
        "#})
        .unwrap();
        let root = tempfile::TempDir::new().unwrap();
        write_cargo(root.path(), "0.0.1");
        let planned = plan(&config, root.path()).unwrap();
        assert!(!planned.has_assets);
        assert_eq!(planned.matrix.include.len(), 0);
        assert_eq!(planned.tag, "v0.0.1");
    }

    #[test]
    fn one_target_is_enough() {
        let config = load(indoc::indoc! {r#"
            [[packages]]
            name = "verctl"
            path = "Cargo.toml"
            [assets]
            targets = ["linux-x64"]
        "#})
        .unwrap();
        let root = tempfile::TempDir::new().unwrap();
        write_cargo(root.path(), "1.2.3");
        let planned = plan(&config, root.path()).unwrap();
        assert!(planned.has_assets);
        assert_eq!(planned.matrix.include.len(), 1);
        assert_eq!(planned.matrix.include[0].id, "linux-x64");
        assert_eq!(planned.matrix.include[0].runner, "ubuntu-latest");
        assert_eq!(
            planned.matrix.include[0].asset,
            "verctl_1.2.3_linux_x64.tar.gz"
        );
    }

    #[test]
    fn runner_override_beats_latest() {
        let config = load(indoc::indoc! {r#"
            [[packages]]
            name = "verctl"
            path = "Cargo.toml"
            [assets]
            targets = [{ id = "darwin-arm64", runner = "macos-15" }]
        "#})
        .unwrap();
        let root = tempfile::TempDir::new().unwrap();
        write_cargo(root.path(), "0.0.1");
        let planned = plan(&config, root.path()).unwrap();
        assert_eq!(planned.matrix.include[0].runner, "macos-15");
    }

    #[test]
    fn two_declared_targets() {
        let config = load(indoc::indoc! {r#"
            [[packages]]
            name = "verctl"
            path = "Cargo.toml"
            [assets]
            bin = "verctl"
            targets = ["darwin-arm64", "linux-x64"]
        "#})
        .unwrap();
        let root = tempfile::TempDir::new().unwrap();
        write_cargo(root.path(), "0.0.1");
        let planned = plan(&config, root.path()).unwrap();
        assert_eq!(
            planned
                .matrix
                .include
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            ["darwin-arm64", "linux-x64"]
        );
        assert_eq!(
            planned.matrix.include[0].asset,
            "verctl_0.0.1_macos_arm64.tar.gz"
        );
        assert_eq!(planned.matrix.include[0].runner, "macos-latest");
    }

    #[test]
    fn unknown_target_needs_a_runner() {
        let config = load(indoc::indoc! {r#"
            [[packages]]
            name = "verctl"
            path = "Cargo.toml"
            [assets]
            targets = ["windows-x64"]
        "#})
        .unwrap();
        let root = tempfile::TempDir::new().unwrap();
        write_cargo(root.path(), "0.0.1");
        let err = plan(&config, root.path()).unwrap_err();
        assert!(format!("{err:#}").contains("runner"), "{err:#}");
    }

    #[test]
    fn github_output_is_one_assignment_per_line() {
        let config = load(indoc::indoc! {r#"
            [[packages]]
            name = "verctl"
            path = "Cargo.toml"
        "#})
        .unwrap();
        let root = tempfile::TempDir::new().unwrap();
        write_cargo(root.path(), "0.0.1");
        let planned = plan(&config, root.path()).unwrap();
        let out = root.path().join("out");
        super::write_github_output(&planned, &out).unwrap();
        let text = std::fs::read_to_string(&out).unwrap();
        assert!(text.contains("has_assets=false\n"), "{text}");
        assert!(text.contains("tag=v0.0.1\n"), "{text}");
        assert!(text.contains("matrix={\"include\":[]}\n"), "{text}");
    }
}
