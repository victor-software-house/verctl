//! Native release assets. Only the targets listed in `[assets]`.
//!
//! Omit `[assets]` (or leave `targets` empty) when a crate has no
//! native binary, or when one host build is enough for every consumer.
//! PR CI never reads this list.

use crate::config::Config;
use crate::github;
use crate::process;
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const BUILD_TIMEOUT: Duration = Duration::from_mins(15);

/// One row of a GitHub Actions `strategy.matrix`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MatrixTarget {
    pub id: String,
    pub runner: String,
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
}

#[derive(Clone, Copy, Debug)]
struct Known {
    id: &'static str,
    runner: &'static str,
    triple: &'static str,
    asset_os_arch: &'static str,
}

const KNOWN: &[Known] = &[
    Known {
        id: "darwin-arm64",
        runner: "macos-14",
        triple: "aarch64-apple-darwin",
        asset_os_arch: "macos_arm64",
    },
    Known {
        id: "linux-x64",
        runner: "ubuntu-24.04",
        triple: "x86_64-unknown-linux-gnu",
        asset_os_arch: "linux_x64",
    },
];

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
    let mut include = Vec::new();
    for id in ids {
        let known = known(id)?;
        include.push(MatrixTarget {
            id: known.id.to_owned(),
            runner: known.runner.to_owned(),
            triple: known.triple.to_owned(),
            asset: format!("{bin}_{version}_{}.tar.gz", known.asset_os_arch),
        });
    }
    Ok(AssetsPlan {
        bin,
        version: version.clone(),
        tag: format!("v{version}"),
        has_assets: !include.is_empty(),
        matrix: Matrix { include },
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
    let rustup = process::argv(&["rustup", "target", "add", &target.triple]);
    process::run_limited(&rustup, &[], BUILD_TIMEOUT).context("rustup target add")?;
    let cargo = process::argv(&[
        "cargo",
        "build",
        "--release",
        "--locked",
        "--target",
        &target.triple,
    ]);
    process::run_limited(&cargo, &[], BUILD_TIMEOUT).context("cargo build --release")?;
    let binary = root
        .join("target")
        .join(&target.triple)
        .join("release")
        .join(&plan.bin);
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

fn known(id: &str) -> Result<&'static Known> {
    KNOWN.iter().find(|known| known.id == id).with_context(|| {
        let ids: Vec<&str> = KNOWN.iter().map(|known| known.id).collect();
        format!("unknown asset target {id:?} (known: {})", ids.join(", "))
    })
}

#[cfg(test)]
mod tests {
    use super::{known, plan};
    use crate::config::Config;
    use anyhow::Result;

    fn load(toml: &str) -> Result<Config> {
        toml::from_str(toml).map_err(Into::into)
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
        std::fs::write(
            root.path().join("Cargo.toml"),
            "[package]\nversion = \"0.0.1\"\n",
        )
        .unwrap();
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
        std::fs::write(
            root.path().join("Cargo.toml"),
            "[package]\nversion = \"1.2.3\"\n",
        )
        .unwrap();
        let planned = plan(&config, root.path()).unwrap();
        assert!(planned.has_assets);
        assert_eq!(planned.matrix.include.len(), 1);
        assert_eq!(planned.matrix.include[0].id, "linux-x64");
        assert_eq!(planned.matrix.include[0].runner, "ubuntu-24.04");
        assert_eq!(
            planned.matrix.include[0].asset,
            "verctl_1.2.3_linux_x64.tar.gz"
        );
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
        std::fs::write(
            root.path().join("Cargo.toml"),
            "[package]\nversion = \"0.0.1\"\n",
        )
        .unwrap();
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
    }

    #[test]
    fn unknown_target_fails() {
        let err = known("windows-x64").unwrap_err();
        assert!(format!("{err:#}").contains("darwin-arm64"), "{err:#}");
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
        std::fs::write(
            root.path().join("Cargo.toml"),
            "[package]\nversion = \"0.0.1\"\n",
        )
        .unwrap();
        let planned = plan(&config, root.path()).unwrap();
        let out = root.path().join("out");
        super::write_github_output(&planned, &out).unwrap();
        let text = std::fs::read_to_string(&out).unwrap();
        assert!(text.contains("has_assets=false\n"), "{text}");
        assert!(text.contains("tag=v0.0.1\n"), "{text}");
        assert!(text.contains("matrix={\"include\":[]}\n"), "{text}");
    }
}
