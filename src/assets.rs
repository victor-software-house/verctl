//! Native release assets. Only the targets listed under `assets`.
//!
//! Each named entry is one target: the platform it produces and the single
//! machine that builds it. One platform is one tarball is one machine, so
//! `runner` names one machine from `runners` and is not a list: two machines
//! under one target would overwrite each other's tarball.
//!
//! Omit `assets` when there is no host binary. Override `prepare`, `build`,
//! and `binary` when the stock rust recipe is the wrong stack, and set `dir`
//! and `files` when a tarball holds more than the binary. PR CI never reads
//! this list.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::config::Config;
use crate::{github, process, runners};

/// Time limit for each build step of one target.
pub const BUILD_TIMEOUT: Duration = Duration::from_mins(15);

/// One row of a GitHub Actions `strategy.matrix`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MatrixTarget {
    /// The target's name under `assets`, such as `linux-x64`.
    pub id: String,
    /// The machine as declared, for the grid. `runs-on:` gets `labels`.
    pub machine: String,
    /// The `runs-on:` labels of the machine that builds this target.
    pub labels: Vec<String>,
    /// Operating system the binary runs on.
    pub os: String,
    /// CPU architecture the binary runs on.
    pub arch: String,
    /// Rust target triple passed to the build.
    pub triple: String,
    /// File name of the release tarball.
    pub asset: String,
}

/// The `strategy.matrix` of the build job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Matrix {
    /// One entry per target.
    pub include: Vec<MatrixTarget>,
}

/// What the assets job builds and uploads, resolved from the config.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssetsPlan {
    /// Name of the binary: the start of each tarball name, and what the
    /// tarball holds unless `dir` and `files` say otherwise.
    pub bin: String,
    /// The first package's current version.
    pub version: String,
    /// The release tag the tarballs are uploaded to.
    pub tag: String,
    /// Whether any target is declared.
    pub has_assets: bool,
    /// The build matrix.
    pub matrix: Matrix,
    #[serde(skip)]
    prepare: Vec<String>,
    #[serde(skip)]
    build: Vec<String>,
    #[serde(skip)]
    pack: Pack,
}

/// What a target's tarball holds, with placeholders still in it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Pack {
    /// The one built binary at this path, packed as `bin`.
    Binary(String),
    /// These paths under `dir`, each kept at its place under `dir`.
    Files { dir: String, files: Vec<String> },
}

/// A platform verctl knows, so a repo that wants the usual thing writes only
/// its name. Every field is a default the repo may override.
#[derive(Clone, Copy, Debug)]
struct Known {
    id: &'static str,
    labels: &'static [&'static str],
    os: &'static str,
    arch: &'static str,
    triple: &'static str,
}

const KNOWN: &[Known] = &[
    Known {
        id: "darwin-arm64",
        labels: &["macos-latest"],
        os: "darwin",
        arch: "arm64",
        triple: "aarch64-apple-darwin",
    },
    Known {
        id: "linux-x64",
        labels: &["ubuntu-latest"],
        os: "linux",
        arch: "x64",
        triple: "x86_64-unknown-linux-gnu",
    },
];

fn known_ids() -> String {
    KNOWN
        .iter()
        .map(|known| known.id)
        .collect::<Vec<_>>()
        .join(", ")
}

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

/// Resolves the declared targets, the first package's version, and its release
/// tag.
///
/// # Errors
///
/// Returns an error when a declared runner is invalid, the config declares no
/// packages, the first package's manifest cannot be read or its version
/// parsed, or the tag template cannot be rendered. It also fails when an asset
/// target names an undeclared runner, or is outside the built-in set without
/// declaring `runner`, `os`, `arch`, and `triple`.
pub fn plan(config: &Config, root: &Path) -> Result<AssetsPlan> {
    runners::declared(config)?;
    let package = config
        .packages
        .first()
        .context("the config declares no packages")?;
    let driver = package.resolve(config, root)?;
    let raw = fs::read_to_string(root.join(&package.path))
        .with_context(|| package.path.display().to_string())?;
    let version = driver.read(&raw)?.trim().to_owned();
    let tag = config.tags.render(
        &version,
        config.tags.uses_name().then_some(package.name.as_str()),
    )?;
    let bin = config
        .assets
        .as_ref()
        .and_then(|assets| assets.bin.clone())
        .unwrap_or_else(|| package.name.clone());
    let assets = config.assets.clone().unwrap_or_default();
    let custom_build = assets.build.is_some();
    let mut include = Vec::new();
    for (id, spec) in &assets.targets {
        include.push(resolve_target(config, id, spec, &bin, &version)?);
    }
    Ok(AssetsPlan {
        bin,
        version: version.clone(),
        tag,
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
        pack: match assets.dir {
            Some(dir) => Pack::Files {
                dir: dir.display().to_string(),
                files: assets
                    .files
                    .iter()
                    .map(|file| file.display().to_string())
                    .collect(),
            },
            None => Pack::Binary(assets.binary.unwrap_or_else(|| STOCK_BINARY.to_owned())),
        },
    })
}

/// The matrix GitHub sees: the job name and the machine's labels, nothing else.
/// `os`, `arch`, and `triple` stay on this side of the boundary.
#[derive(Serialize)]
struct GithubRow {
    id: String,
    labels: Vec<String>,
}

#[derive(Serialize)]
struct GithubMatrix {
    include: Vec<GithubRow>,
}

/// Appends `has_assets`, `tag`, and the GitHub-facing matrix to the step output
/// file `path`.
///
/// # Errors
///
/// Returns an error when the matrix cannot be encoded as JSON, or when the
/// file at `path` cannot be opened or written.
pub fn write_github_output(plan: &AssetsPlan, path: &Path) -> Result<()> {
    let include = plan
        .matrix
        .include
        .iter()
        .map(|row| GithubRow {
            id: row.id.clone(),
            labels: row.labels.clone(),
        })
        .collect();
    write_matrix_output(path, plan.has_assets, &plan.tag, include)
}

/// One writer for both kinds of set, so a workflow reads `has_assets`, `tag`,
/// and `matrix` the same way from either.
fn write_matrix_output(
    path: &Path,
    has_assets: bool,
    tag: &str,
    include: Vec<GithubRow>,
) -> Result<()> {
    use ctl_core::formatdoc;
    let encoded = serde_json::to_string(&GithubMatrix { include }).context("encode matrix")?;
    let body = formatdoc! {"
        has_assets={has_assets}
        tag={tag}
        matrix={encoded}
    "};
    github::write_output(path, &body)
}

/// Builds the target `id` and packs its binary, or the `files` under `dir`,
/// into a tarball under `root`.
///
/// # Errors
///
/// Returns an error when `id` is not a target in `plan`, when the prepare,
/// build, or `tar` command cannot run, times out, or exits unsuccessfully, or
/// when the release binary, or a path `files` lists, is missing after the
/// build. It also fails when a path is not valid UTF-8.
pub fn build(plan: &AssetsPlan, id: &str, root: &Path) -> Result<PathBuf> {
    let target = plan
        .matrix
        .include
        .iter()
        .find(|row| row.id == id)
        .with_context(|| format!("target {id:?} is not in assets"))?;
    let ctx = target_ctx(plan, target);
    if !plan.prepare.is_empty() {
        let prepare = expand(&plan.prepare, &ctx);
        process::run_inherit(&prepare, BUILD_TIMEOUT).context("assets.prepare")?;
    }
    let build = expand(&plan.build, &ctx);
    process::run_inherit(&build, BUILD_TIMEOUT).context("assets.build")?;
    let (dir, files) = match &plan.pack {
        Pack::Binary(binary) => {
            let binary = root.join(expand_one(binary, &ctx));
            anyhow::ensure!(
                binary.is_file(),
                "missing release binary {}",
                binary.display()
            );
            let dir = binary.parent().context("binary parent")?.to_path_buf();
            (dir, vec![plan.bin.clone()])
        }
        Pack::Files { dir, files } => {
            let dir = root.join(expand_one(dir, &ctx));
            let files = expand(files, &ctx);
            for file in &files {
                let packed = dir.join(file);
                anyhow::ensure!(
                    packed.exists(),
                    "assets.files: {} is missing after the build",
                    packed.display()
                );
            }
            (dir, files)
        }
    };
    let tarball = root.join(&target.asset);
    let mut tar = process::argv(&[
        "tar",
        "czf",
        tarball.to_str().context("tarball path")?,
        "-C",
        dir.to_str().context("dir")?,
    ]);
    tar.extend(files);
    process::run_inherit(&tar, Duration::from_mins(2)).context("tar")?;
    Ok(tarball)
}

/// Uploads `tarball` to the release `tag` and returns the asset's download URL.
///
/// # Errors
///
/// Returns an error when neither `GITHUB_TOKEN` nor `GH_TOKEN` is set, when
/// the GitHub repository cannot be determined, or when the upload fails, as
/// described for [`crate::github::upload_release_asset`].
pub fn upload(root: &Path, tag: &str, tarball: &Path) -> Result<String> {
    let token = crate::release::resolve_token()?;
    let repo = github::repo(root)?;
    github::upload_release_asset(&token, &repo, tag, tarball)
}

/// One target of a vendored set: its platform name, the machine that builds
/// it, and the archive it writes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VendoredTarget {
    /// The target's key under `targets`, such as `linux-x64`: `{id}`.
    pub id: String,
    /// The machine as declared, for the grid. `runs-on:` gets `labels`.
    pub machine: String,
    /// The `runs-on:` labels of the machine that builds this target.
    pub labels: Vec<String>,
    /// File name of the archive.
    pub asset: String,
}

/// What a vendored set builds and uploads, resolved from the config.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VendoredPlan {
    /// The set's key under `vendored`: `{name}`.
    pub name: String,
    /// The upstream version the set declares.
    pub version: String,
    /// The release tag the archives are uploaded to.
    pub tag: String,
    /// One entry per target.
    pub targets: Vec<VendoredTarget>,
    #[serde(skip)]
    build: Vec<String>,
    #[serde(skip)]
    dir: String,
    #[serde(skip)]
    files: Vec<String>,
    #[serde(skip)]
    notes: Option<PathBuf>,
}

/// Where a vendored target's archive and checksum landed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VendoredUpload {
    /// The Release's HTML URL.
    pub release: String,
    /// The archive's download URL, or `already <name>`.
    pub archive: String,
    /// The checksum file's download URL, or `already <name>`.
    pub checksum: String,
}

/// Resolves the vendored set `name`: its tag, and each target's machine and
/// archive name.
///
/// # Errors
///
/// Returns an error when `vendored` declares no set `name`, or when a target's
/// machine declares an invalid label set.
pub fn plan_vendored(config: &Config, name: &str) -> Result<VendoredPlan> {
    let set = config.vendored.get(name).with_context(|| {
        format!(
            "vendored declares no set {name:?} (declared: {})",
            config
                .vendored
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        )
    })?;
    let mut targets = Vec::new();
    for (id, target) in &set.targets {
        let machine = runners::one(config, "vendored target", id, &target.runner)?;
        targets.push(VendoredTarget {
            id: id.clone(),
            machine: machine.name,
            labels: machine.labels,
            asset: expand_one(&set.archive, &vendored_ctx(name, &set.version, id)),
        });
    }
    Ok(VendoredPlan {
        name: name.to_owned(),
        version: set.version.clone(),
        tag: expand_one(&set.tag, &vendored_ctx(name, &set.version, "")),
        targets,
        build: set.build.clone(),
        dir: set.dir.display().to_string(),
        files: set
            .files
            .iter()
            .map(|file| file.display().to_string())
            .collect(),
        notes: set.notes.clone(),
    })
}

/// The placeholders a vendored template may use. The tag names one Release
/// for every target, so it is rendered with an empty `id`.
fn vendored_ctx(name: &str, version: &str, id: &str) -> BTreeMap<&'static str, String> {
    BTreeMap::from([
        ("name", name.to_owned()),
        ("version", version.to_owned()),
        ("id", id.to_owned()),
    ])
}

/// Appends `has_assets`, `tag`, and the matrix of a vendored set to the step
/// output file `path`, in the same shape as [`write_github_output`].
///
/// # Errors
///
/// Returns an error when the matrix cannot be encoded as JSON, or when the
/// file at `path` cannot be opened or written.
pub fn write_vendored_github_output(plan: &VendoredPlan, path: &Path) -> Result<()> {
    let include = plan
        .targets
        .iter()
        .map(|target| GithubRow {
            id: target.id.clone(),
            labels: target.labels.clone(),
        })
        .collect();
    write_matrix_output(path, !plan.targets.is_empty(), &plan.tag, include)
}

/// Runs the set's `build` for target `id`, then packs the listed files from
/// `dir` into the target's archive under `root`, each at its place under
/// `dir`.
///
/// # Errors
///
/// Returns an error when `id` is not a target of the set, when the build or
/// `tar` cannot run, times out, or exits unsuccessfully, when a listed file is
/// missing after the build, or when a path is not valid UTF-8.
pub fn build_vendored(plan: &VendoredPlan, id: &str, root: &Path) -> Result<PathBuf> {
    let target = plan
        .targets
        .iter()
        .find(|target| target.id == id)
        .with_context(|| format!("target {id:?} is not in vendored.{}", plan.name))?;
    let ctx = vendored_ctx(&plan.name, &plan.version, id);
    if !plan.build.is_empty() {
        process::run_inherit(&expand(&plan.build, &ctx), BUILD_TIMEOUT)
            .with_context(|| format!("vendored.{}.build", plan.name))?;
    }
    let dir = root.join(expand_one(&plan.dir, &ctx));
    let files = expand(&plan.files, &ctx);
    for file in &files {
        let packed = dir.join(file);
        anyhow::ensure!(
            packed.exists(),
            "vendored.{}: {} is missing after the build",
            plan.name,
            packed.display()
        );
    }
    let archive = root.join(&target.asset);
    let mut tar = process::argv(&[
        "tar",
        "czf",
        archive.to_str().context("archive path")?,
        "-C",
        dir.to_str().context("dir")?,
    ]);
    tar.extend(files);
    process::run_inherit(&tar, Duration::from_mins(2)).context("tar")?;
    Ok(archive)
}

/// Creates the set's Release at HEAD when it is missing, uploads `archive`,
/// then uploads `<archive>.sha256` written from the digest GitHub reports for
/// the stored archive. An asset the Release already holds is reported as
/// `already`.
///
/// # Errors
///
/// Returns an error when no token is set, the GitHub repository or HEAD cannot
/// be read, the notes file cannot be read, the Release cannot be read or
/// created, an upload fails, GitHub reports no digest for the archive, or the
/// checksum file cannot be written.
pub fn upload_vendored(plan: &VendoredPlan, root: &Path, archive: &Path) -> Result<VendoredUpload> {
    let token = crate::release::resolve_token()?;
    let repo = github::repo(root)?;
    let notes = match &plan.notes {
        Some(path) => fs::read_to_string(root.join(path))
            .with_context(|| format!("vendored.{}.notes {}", plan.name, path.display()))?,
        None => String::new(),
    };
    let sha = crate::git::head_sha(root)?;
    let release = github::ensure_vendored_release(&token, &repo, &plan.tag, &notes, &sha)?;
    let uploaded = github::upload_release_asset(&token, &repo, &plan.tag, archive)?;
    let name = archive
        .file_name()
        .and_then(|name| name.to_str())
        .context("archive name")?;
    let digest = github::asset_sha256(&token, &repo, &plan.tag, name)?;
    let checksum = archive.with_file_name(format!("{name}.sha256"));
    fs::write(&checksum, checksum_line(&digest, name))
        .with_context(|| checksum.display().to_string())?;
    let checksum_uploaded = github::upload_release_asset(&token, &repo, &plan.tag, &checksum)?;
    Ok(VendoredUpload {
        release,
        archive: uploaded,
        checksum: checksum_uploaded,
    })
}

/// One `sha256sum` line: the digest, two spaces, the file name.
fn checksum_line(digest: &str, name: &str) -> String {
    format!("{digest}  {name}\n")
}

fn resolve_target(
    config: &Config,
    id: &str,
    spec: &crate::config::AssetTarget,
    bin: &str,
    version: &str,
) -> Result<MatrixTarget> {
    let known = KNOWN.iter().find(|known| known.id == id);
    // A target outside the built-in set describes a platform verctl knows
    // nothing about, so it must describe all of it. Partial records used to
    // pass and reach `cargo build --target ""`.
    let Some(known) = known else {
        let (Some(runner), Some(os), Some(arch), Some(triple)) = (
            spec.runner.as_deref(),
            spec.os.as_deref(),
            spec.arch.as_deref(),
            spec.triple.as_deref(),
        ) else {
            bail!(
                "unknown asset target {id:?} needs runner, os, arch, triple (or use {})",
                known_ids()
            );
        };
        let machine = runners::one(config, "asset target", id, runner)?;
        return Ok(MatrixTarget {
            id: id.to_owned(),
            machine: machine.name,
            labels: machine.labels,
            os: os.to_owned(),
            arch: arch.to_owned(),
            triple: triple.to_owned(),
            asset: asset_name(bin, version, os, arch),
        });
    };
    let (machine, labels) = match spec.runner.as_deref() {
        Some(runner) => {
            let machine = runners::one(config, "asset target", id, runner)?;
            (machine.name, machine.labels)
        }
        None => (known.labels.join(", "), strings(known.labels)),
    };
    let os = spec.os.as_deref().unwrap_or(known.os);
    let arch = spec.arch.as_deref().unwrap_or(known.arch);
    let triple = spec.triple.as_deref().unwrap_or(known.triple);
    Ok(MatrixTarget {
        id: id.to_owned(),
        machine,
        labels,
        os: os.to_owned(),
        arch: arch.to_owned(),
        triple: triple.to_owned(),
        asset: asset_name(bin, version, os, arch),
    })
}

/// `os = "darwin"` renders as `macos` in the filename. The one rename.
fn asset_name(bin: &str, version: &str, os: &str, arch: &str) -> String {
    let os = if os == "darwin" { "macos" } else { os };
    format!("{bin}_{version}_{os}_{arch}.tar.gz")
}

fn strings(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
}

fn target_ctx(plan: &AssetsPlan, target: &MatrixTarget) -> BTreeMap<&'static str, String> {
    BTreeMap::from([
        ("bin", plan.bin.clone()),
        ("version", plan.version.clone()),
        ("id", target.id.clone()),
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
    use anyhow::Result;

    use super::plan;
    use crate::config::Config;

    fn load(yaml: &str) -> Result<Config> {
        Config::parse(yaml)
    }

    const REGISTRY: &str = indoc::indoc! {"
        packages:
          - name: verctl
            path: Cargo.toml

        runners:
          big:
            labels: [self-hosted, linux, x64]
          macos15:
            labels: [macos-15]
          arm:
            labels: [ubuntu-24.04-arm]
    "};

    fn with(extra: &str) -> Result<Config> {
        load(&format!("{REGISTRY}{extra}"))
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

    fn planned(config: &Config, version: &str) -> Result<super::AssetsPlan> {
        let root = tempfile::TempDir::new().unwrap();
        write_cargo(root.path(), version);
        plan(config, root.path())
    }

    /// The format `sha256sum --check` reads: digest, two spaces, file name.
    #[test]
    fn a_checksum_line_is_what_sha256sum_reads() {
        assert_eq!(
            super::checksum_line("abc123", "lib-1.0.0-linux-x64.tar.gz"),
            "abc123  lib-1.0.0-linux-x64.tar.gz\n"
        );
    }

    #[test]
    fn omitted_assets_means_no_native_jobs() {
        let config = load(indoc::indoc! {"
            packages:
              - name: ctl-core
                path: Cargo.toml
        "})
        .unwrap();
        let plan = planned(&config, "0.0.1").unwrap();
        assert!(!plan.has_assets);
        assert_eq!(plan.matrix.include.len(), 0);
        assert_eq!(plan.tag, "v0.0.1");
    }

    #[test]
    fn assets_tag_follows_the_declared_template() {
        let config = load(indoc::indoc! {r#"
            packages:
              - name: ctl-core
                path: Cargo.toml
            tags:
              template: "{name}@{version}"
            assets:
              linux-x64: {}
        "#})
        .unwrap();
        let plan = planned(&config, "0.0.1").unwrap();
        assert_eq!(plan.tag, "ctl-core@0.0.1");
    }

    #[test]
    fn a_built_in_target_needs_only_its_name() {
        let config = with("assets:\n  linux-x64: {}\n").unwrap();
        let plan = planned(&config, "1.2.3").unwrap();
        assert!(plan.has_assets);
        assert_eq!(plan.matrix.include.len(), 1);
        assert_eq!(plan.matrix.include[0].id, "linux-x64");
        assert_eq!(plan.matrix.include[0].labels, ["ubuntu-latest"]);
        assert_eq!(
            plan.matrix.include[0].asset,
            "verctl_1.2.3_linux_x64.tar.gz"
        );
    }

    #[test]
    fn a_declared_machine_beats_the_built_in_one() {
        let config = with(indoc::indoc! {"
            assets:
              darwin-arm64:
                runner: macos15
        "})
        .unwrap();
        let plan = planned(&config, "0.0.1").unwrap();
        assert_eq!(plan.matrix.include[0].labels, ["macos-15"]);
        assert_eq!(plan.matrix.include[0].machine, "macos15");
        assert_eq!(plan.matrix.include[0].triple, "aarch64-apple-darwin");
    }

    #[test]
    fn a_multi_label_machine_reaches_the_matrix_whole() {
        let config = with(indoc::indoc! {"
            assets:
              linux-x64:
                runner: big
        "})
        .unwrap();
        let plan = planned(&config, "0.0.1").unwrap();
        assert_eq!(
            plan.matrix.include[0].labels,
            ["self-hosted", "linux", "x64"]
        );
    }

    #[test]
    fn a_user_defined_target_needs_the_whole_record() {
        let config = with(indoc::indoc! {"
            assets:
              linux-arm64:
                runner: arm
                os: linux
                arch: arm64
                triple: aarch64-unknown-linux-gnu
        "})
        .unwrap();
        let plan = planned(&config, "0.0.2").unwrap();
        assert_eq!(plan.matrix.include[0].triple, "aarch64-unknown-linux-gnu");
        assert_eq!(plan.matrix.include[0].labels, ["ubuntu-24.04-arm"]);
        assert_eq!(
            plan.matrix.include[0].asset,
            "verctl_0.0.2_linux_arm64.tar.gz"
        );
    }

    #[test]
    fn a_partial_unknown_target_no_longer_builds_an_empty_triple() {
        let config = with(indoc::indoc! {"
            assets:
              linux-arm64:
                runner: arm
        "})
        .unwrap();
        let err = planned(&config, "0.0.1").unwrap_err();
        assert!(
            format!("{err:#}").contains("runner, os, arch, triple"),
            "{err:#}"
        );
    }

    #[test]
    fn an_unknown_target_alone_names_the_built_ins() {
        let config = with("assets:\n  windows-x64: {}\n").unwrap();
        let err = planned(&config, "0.0.1").unwrap_err();
        let text = format!("{err:#}");
        assert!(text.contains("runner, os, arch, triple"), "{text}");
        assert!(text.contains("darwin-arm64, linux-x64"), "{text}");
    }

    #[test]
    fn an_undeclared_machine_is_rejected() {
        let config = with(indoc::indoc! {"
            assets:
              linux-x64:
                runner: nope
        "})
        .unwrap();
        let err = planned(&config, "0.0.1").unwrap_err();
        let text = format!("{err:#}");
        assert!(text.contains("not declared in runners"), "{text}");
        assert!(text.contains("arm, big, macos15"), "{text}");
    }

    #[test]
    fn a_target_takes_no_runners_list() {
        let err = Config::parse(&format!(
            "{REGISTRY}{}",
            indoc::indoc! {"
                assets:
                  linux-x64:
                    runners: [big]
            "}
        ))
        .unwrap_err();
        assert!(format!("{err:#}").contains("runners"), "{err:#}");
    }

    #[test]
    fn two_targets_and_the_shared_recipe() {
        let config = with(indoc::indoc! {"
            assets:
              bin: verctl
              darwin-arm64: {}
              linux-x64: {}
        "})
        .unwrap();
        let plan = planned(&config, "0.0.1").unwrap();
        assert_eq!(
            plan.matrix
                .include
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            ["darwin-arm64", "linux-x64"]
        );
        assert_eq!(
            plan.matrix.include[0].asset,
            "verctl_0.0.1_macos_arm64.tar.gz"
        );
        assert_eq!(plan.matrix.include[0].labels, ["macos-latest"]);
    }

    #[test]
    fn github_matrix_is_only_id_and_labels() {
        let config = with(indoc::indoc! {"
            assets:
              linux-x64: {}
              darwin-arm64: {}
        "})
        .unwrap();
        let root = tempfile::TempDir::new().unwrap();
        write_cargo(root.path(), "0.0.1");
        let plan = plan(&config, root.path()).unwrap();
        let out = root.path().join("out");
        super::write_github_output(&plan, &out).unwrap();
        let text = std::fs::read_to_string(&out).unwrap();
        let matrix_line = text
            .lines()
            .find(|line| line.starts_with("matrix="))
            .unwrap();
        let json: serde_json::Value =
            serde_json::from_str(matrix_line.trim_start_matches("matrix=")).unwrap();
        let first = &json["include"][0];
        assert_eq!(first["id"], "darwin-arm64");
        assert_eq!(first["labels"][0], "macos-latest");
        assert!(first.get("triple").is_none(), "{first}");
        assert!(first.get("asset").is_none(), "{first}");
        assert!(first.get("machine").is_none(), "{first}");
        assert!(text.contains("has_assets=true\n"), "{text}");
    }

    #[test]
    fn github_output_is_one_assignment_per_line() {
        let config = load(indoc::indoc! {"
            packages:
              - name: verctl
                path: Cargo.toml
        "})
        .unwrap();
        let root = tempfile::TempDir::new().unwrap();
        write_cargo(root.path(), "0.0.1");
        let plan = plan(&config, root.path()).unwrap();
        let out = root.path().join("out");
        super::write_github_output(&plan, &out).unwrap();
        let text = std::fs::read_to_string(&out).unwrap();
        assert!(text.contains("has_assets=false\n"), "{text}");
        assert!(text.contains("tag=v0.0.1\n"), "{text}");
        assert!(text.contains("matrix={\"include\":[]}"), "{text}");
        assert!(!text.contains("triple"), "{text}");
    }
}
