//! Check each crates.io crate's public API at the release type its pending
//! release allows.
//!
//! A release is a patch by default, and a real API break bumps the minor. So
//! a crate whose next release is a patch must not break its API: `semver`
//! runs `cargo semver-checks --release-type minor`, which runs only the lints
//! that need a major release in Cargo's terms. A crate whose next release
//! bumps the minor may break it, so `semver` skips it. The rule covers 0.x
//! only: at 1.0 a break needs a major release, and reaching 1.0 is an
//! operator decision that revisits the rule, so a crate at 1.0 or later
//! fails the run. A crate cargo-semver-checks cannot check, one with no
//! library target or a proc-macro, gets no row, and the 1.0 rule, which is
//! about API breaks, does not apply to it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use semver::Version;
use serde::Serialize;
use toml_edit::DocumentMut;

use crate::config::Config;
use crate::fragment::{Bump, Fragment};
use crate::registry::{self, RegistryCheck};
use crate::{docsrs, process, publish, publisher};

/// How long one crate's check may run. It builds the rustdoc of the current
/// crate and of its baseline, so it sits as high as a cold test build.
pub const CHECK_TIMEOUT: Duration = Duration::from_mins(30);

/// What `semver` decided for one crate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Decision {
    /// Checked with `--release-type minor`, and no lint failed.
    Passed,
    /// Checked with `--release-type minor`, and a lint failed.
    Failed,
    /// A pending fragment bumps the package's minor or major.
    MinorPending,
    /// The manifest's major.minor differs from the baseline's: the Version PR
    /// already moved it.
    VersionMoved,
    /// crates.io has no release to compare against.
    Unpublished,
}

/// One crate's row in the `semver` report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct SemverLine {
    /// The crate name crates.io knows.
    pub name: String,
    /// The verctl package that releases it.
    pub package: String,
    /// The version in its manifest.
    pub version: String,
    /// The release it is compared against, when crates.io has one.
    pub baseline: Option<String>,
    /// What `semver` decided.
    pub decision: Decision,
}

/// One crate before its check runs.
struct Planned {
    /// Its row, whose decision is final when `check` is false.
    line: SemverLine,
    manifest: PathBuf,
    /// Whether to run the check.
    check: bool,
}

/// Check every crates.io crate whose next release is a patch, and report
/// every crate with the decision taken.
///
/// A package's next release is a patch unless a fragment in `fragments`
/// bumps it further, or its manifest's major.minor already differs from the
/// crates.io baseline. A workspace package counts once per publishing
/// member, and a crate `checkable` rejects is left out.
///
/// # Errors
///
/// Returns an error when the plan, a manifest, a publisher, or the crates.io
/// index cannot be read, when a crate is at 1.0 or later, or when any check
/// fails or cannot run, naming each failed crate.
pub fn run(config: &Config, root: &Path, fragments: &[Fragment]) -> Result<Vec<SemverLine>> {
    let planned = plan(config, root, fragments)?;
    let mut lines = Vec::with_capacity(planned.len());
    let mut failed = Vec::new();
    for Planned {
        mut line,
        manifest,
        check,
    } in planned
    {
        if check {
            let baseline = line.baseline.clone().unwrap_or_default();
            let argv = process::argv(&[
                "cargo",
                "semver-checks",
                "--manifest-path",
                &manifest.display().to_string(),
                "--package",
                &line.name,
                "--baseline-version",
                &baseline,
                "--release-type",
                "minor",
            ]);
            line.decision = match process::run_inherit(&argv, CHECK_TIMEOUT) {
                Ok(()) => Decision::Passed,
                Err(error) => {
                    // The full output is already on stderr; the first line
                    // says whether the tool exited, timed out, or never ran.
                    let error = format!("{error:#}");
                    let cause = error.lines().next().unwrap_or_default();
                    failed.push(format!("{}@{} ({cause})", line.name, line.version));
                    Decision::Failed
                }
            };
        }
        lines.push(line);
    }
    if !failed.is_empty() {
        bail!(
            "cargo semver-checks did not pass for {}. A failed lint is an API break in a crate whose next release is a patch: add a minor fragment if it is intended. Any other failure is the tool's, shown above",
            failed.join(", ")
        );
    }
    Ok(lines)
}

fn plan(config: &Config, root: &Path, fragments: &[Fragment]) -> Result<Vec<Planned>> {
    let planned = publish::plan(config, root)?;
    let mut crates = Vec::new();
    for (spec, entry) in config.packages.iter().zip(&planned.packages) {
        if entry.argv.is_empty() {
            continue;
        }
        let bump = pending_bump(fragments, &entry.name);
        for upload in publisher::uploads(config, spec, root, entry)? {
            let RegistryCheck::CratesIo { name } = &upload.check else {
                continue;
            };
            if !checkable(&upload.manifest)? {
                continue;
            }
            let version = Version::parse(&upload.version)
                .with_context(|| format!("{name}: semver {:?}", upload.version))?;
            let baseline = registry::crates_io_baseline(registry::CRATES_IO_INDEX, name, &version)?;
            let skip = skip_reason(name, bump, &version, baseline.as_ref())?;
            crates.push(Planned {
                line: SemverLine {
                    name: name.clone(),
                    package: entry.name.clone(),
                    version: upload.version.clone(),
                    baseline: baseline.as_ref().map(Version::to_string),
                    decision: skip.clone().unwrap_or(Decision::Passed),
                },
                manifest: upload.manifest,
                check: skip.is_none(),
            });
        }
    }
    Ok(crates)
}

/// Whether cargo-semver-checks can check the crate in `manifest`: it has a
/// library target, as [`docsrs::has_library`] resolves it, that is not a
/// proc-macro. cargo-semver-checks 0.51.0 leaves out a proc-macro
/// (`is_lib_like_checkable_target`) and refuses to run with nothing left.
fn checkable(manifest: &Path) -> Result<bool> {
    if !docsrs::has_library(manifest)? {
        return Ok(false);
    }
    let raw = std::fs::read_to_string(manifest).with_context(|| manifest.display().to_string())?;
    let document = raw
        .parse::<DocumentMut>()
        .with_context(|| manifest.display().to_string())?;
    // Cargo still reads the old `proc_macro` spelling.
    let proc_macro = ["proc-macro", "proc_macro"].iter().any(|key| {
        document
            .get("lib")
            .and_then(|lib| lib.get(key))
            .and_then(toml_edit::Item::as_bool)
            == Some(true)
    });
    Ok(!proc_macro)
}

/// The largest bump any fragment asks for `package`, or [`Bump::None`].
fn pending_bump(fragments: &[Fragment], package: &str) -> Bump {
    fragments
        .iter()
        .flat_map(|fragment| &fragment.packages)
        .filter(|named| named.name == package)
        .map(|named| named.bump)
        .max()
        .unwrap_or(Bump::None)
}

/// Why a crate at `version` whose package has `bump` pending is not checked
/// against `baseline`, or `None` when it is checked.
fn skip_reason(
    name: &str,
    bump: Bump,
    version: &Version,
    baseline: Option<&Version>,
) -> Result<Option<Decision>> {
    if version.major >= 1 {
        bail!(
            "{name} is at {version}: the release-type rule covers 0.x only, and reaching 1.0 needs the rule revisited"
        );
    }
    let Some(baseline) = baseline else {
        return Ok(Some(Decision::Unpublished));
    };
    if bump >= Bump::Minor {
        return Ok(Some(Decision::MinorPending));
    }
    if (baseline.major, baseline.minor) != (version.major, version.minor) {
        return Ok(Some(Decision::VersionMoved));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use semver::Version;
    use tempfile::TempDir;

    use super::{Decision, checkable, pending_bump, skip_reason};
    use crate::fragment::{Bump, Fragment, PackageBump};

    fn version(raw: &str) -> Version {
        Version::parse(raw).unwrap()
    }

    fn skip(bump: Bump, current: &str, baseline: Option<&str>) -> Option<Decision> {
        skip_reason(
            "demo",
            bump,
            &version(current),
            baseline.map(version).as_ref(),
        )
        .unwrap()
    }

    #[test]
    fn a_patch_release_is_checked_at_0_0_z_and_0_y_z() {
        assert_eq!(skip(Bump::Patch, "0.0.2", Some("0.0.2")), None);
        assert_eq!(skip(Bump::None, "0.4.10", Some("0.4.10")), None);
        assert_eq!(skip(Bump::Patch, "0.4.11", Some("0.4.10")), None);
    }

    #[test]
    fn a_pending_minor_fragment_skips_the_check() {
        assert_eq!(
            skip(Bump::Minor, "0.0.2", Some("0.0.2")),
            Some(Decision::MinorPending)
        );
    }

    #[test]
    fn a_version_pr_that_moved_the_minor_skips_the_check() {
        assert_eq!(
            skip(Bump::None, "0.1.0", Some("0.0.2")),
            Some(Decision::VersionMoved)
        );
        assert_eq!(
            skip(Bump::None, "0.5.0", Some("0.4.10")),
            Some(Decision::VersionMoved)
        );
    }

    #[test]
    fn a_crate_with_no_release_is_not_checked() {
        assert_eq!(
            skip(Bump::Patch, "0.0.1", None),
            Some(Decision::Unpublished)
        );
    }

    #[test]
    fn only_a_library_that_is_not_a_proc_macro_is_checkable() {
        let crate_with = |manifest: &str, source: &str| {
            let dir = TempDir::new().unwrap();
            fs::create_dir_all(dir.path().join("src")).unwrap();
            fs::write(dir.path().join(source), "").unwrap();
            fs::write(dir.path().join("Cargo.toml"), manifest).unwrap();
            dir
        };
        let package = "[package]\nname = \"demo\"\nversion = \"0.0.2\"\n";
        let cases = [
            ("a library", package.to_owned(), "src/lib.rs", true),
            ("a binary", package.to_owned(), "src/main.rs", false),
            (
                "a proc-macro",
                format!("{package}\n[lib]\nproc-macro = true\n"),
                "src/lib.rs",
                false,
            ),
            (
                "the old proc_macro spelling",
                format!("{package}\n[lib]\nproc_macro = true\n"),
                "src/lib.rs",
                false,
            ),
        ];
        for (label, manifest, source, expected) in cases {
            let dir = crate_with(&manifest, source);
            assert_eq!(
                checkable(&dir.path().join("Cargo.toml")).unwrap(),
                expected,
                "{label}"
            );
        }
    }

    #[test]
    fn a_crate_at_1_0_fails() {
        let error = skip_reason("demo", Bump::Patch, &version("1.0.0"), None).unwrap_err();
        assert!(format!("{error:#}").contains("1.0"), "{error:#}");
    }

    #[test]
    fn the_largest_bump_for_the_package_wins() {
        let fragment = |name: &str, bump: Bump| Fragment {
            path: "f.md".into(),
            packages: vec![PackageBump {
                name: name.into(),
                bump,
            }],
            summary: String::new(),
            group: None,
        };
        let fragments = [
            fragment("demo", Bump::Patch),
            fragment("demo", Bump::Minor),
            fragment("other", Bump::Major),
        ];
        assert_eq!(pending_bump(&fragments, "demo"), Bump::Minor);
        assert_eq!(pending_bump(&fragments, "none"), Bump::None);
    }
}
