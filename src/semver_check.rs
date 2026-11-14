//! Check each crates.io crate's public API at the release type its pending
//! release allows.
//!
//! A release is a patch by default, and a real API break bumps the minor.
//! A crate whose next release is a patch must therefore not break its API.
//! For it, `semver` runs `cargo semver-checks --release-type minor`, which
//! runs only the lints that need a major release in Cargo's terms.
//! A crate whose next release bumps the minor may break its API, so `semver`
//! skips it.
//!
//! The rule covers 0.x only.
//! At 1.0 a break needs a major release.
//! Reaching 1.0 is an operator decision that revisits the rule, so a crate at
//! 1.0 or later fails the run.
//!
//! A crate that cargo-semver-checks cannot check gets no row.
//! That covers a crate with no library target and a proc-macro.
//! The 1.0 rule is about API breaks, so it does not apply to such a crate.

use std::collections::BTreeSet;
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{env, fs};

use anyhow::{Context, Result, bail};
use semver::Version;
use serde::Serialize;
use toml_edit::DocumentMut;

use crate::config::Config;
use crate::fragment::{Bump, Fragment};
use crate::registry::{self, RegistryCheck};
use crate::{process, publish, publisher, release};

/// How long one crate's check may run. It builds the rustdoc of the current
/// crate and of its baseline, so it sits as high as a cold test build.
pub const CHECK_TIMEOUT: Duration = Duration::from_mins(30);

/// What `semver` decided for one crate.
///
/// Not `#[non_exhaustive]`: the binary prints a label for every variant, and
/// an exhaustive match makes a new variant fail the build until it has one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Decision {
    /// Checked with `--release-type minor`, and no lint failed.
    Passed,
    /// Checked with `--release-type minor`, and a lint failed.
    Failed,
    /// A pending fragment bumps the package's minor or major.
    MinorPending,
    /// The package's release is a patch that a `patch!` fragment marks. Either
    /// the fragment is pending, or the Version PR consumed it and gave the
    /// version's CHANGELOG section the Breaking heading. The API break ships
    /// as a patch on purpose, for that release.
    BreakingPatch,
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
    /// The crate name on crates.io.
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
/// The next release of a package is a patch unless a fragment in `fragments`
/// bumps it further.
/// It is also not a patch when the major.minor in its manifest already differs
/// from the crates.io baseline.
/// Each publishing member counts a workspace package once.
/// The report leaves out any crate that `checkable` rejects.
///
/// # Errors
///
/// Returns an error when:
///
/// - The plan, a manifest, a publisher, or the crates.io index cannot be read.
/// - A crate is at 1.0 or later.
/// - Any check fails or cannot run, in which case the error names each failed
///   crate.
pub fn run(config: &Config, root: &Path, fragments: &[Fragment]) -> Result<Vec<SemverLine>> {
    let planned = plan(config, root, fragments)?;
    let logins = trusted_logins();
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
            line.decision = match check_crate(&argv, &line.name, &line.version, &logins) {
                Ok(()) => Decision::Passed,
                Err(detail) => {
                    failed.push(format!("{}@{} {detail}", line.name, line.version));
                    Decision::Failed
                }
            };
        }
        lines.push(line);
    }
    if !failed.is_empty() {
        bail!(
            "cargo semver-checks did not pass for {}. A failed lint is an API break in a crate whose next release is a patch: if it is intended, add a minor fragment, or bump the package by `patch!` to ship it in this patch. Any other failure is the tool's, shown above",
            failed.join(", ")
        );
    }
    Ok(lines)
}

/// The variable that names the crates.io logins `semver` trusts, as the
/// templates repository renders it into `mise.toml`.
const TRUSTED_OWNERS: &str = "CARGO_TRUSTED_OWNERS";

/// The crates.io logins in `CARGO_TRUSTED_OWNERS`, split on whitespace, or
/// none when it is unset.
fn trusted_logins() -> Vec<String> {
    env::var(TRUSTED_OWNERS)
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

/// Run one crate's check. A failure returns the text that follows
/// `name@version` in the error: the tool's first line of failure, and why
/// no second run followed when the cooldown rejected a crate.
///
/// When the publish-age cooldown rejected only crates that `logins` own on
/// crates.io, the check runs once more without the cooldown, and that run
/// decides.
fn check_crate(
    argv: &[String],
    name: &str,
    version: &str,
    logins: &[String],
) -> Result<(), String> {
    let Err(error) = process::run_inherit(argv, CHECK_TIMEOUT) else {
        return Ok(());
    };
    // The full output is already on stderr; the first line shows whether the
    // tool exited, timed out, or never ran.
    let error = format!("{error:#}");
    let cause = first_line(&error);
    let rejected = cooldown_rejections(&error);
    if rejected.is_empty() {
        return Err(format!("({cause})"));
    }
    let owners = |crate_name: &str| {
        registry::crates_io_user_owners(registry::CRATES_IO_API, crate_name)
            .map(Option::unwrap_or_default)
    };
    match retry_decision(&rejected, logins, owners) {
        Err(lookup) => Err(format!("({cause}) (the owners lookup failed: {lookup:#})")),
        Ok(Retry::NoRetry(reason)) => Err(format!("({cause}) ({reason})")),
        Ok(Retry::Allow) => {
            let _ = writeln!(
                io::stderr(),
                "verctl: cargo semver-checks for {name}@{version} hit the publish-age cooldown on {}, all owned by {TRUSTED_OWNERS}; checking again without it",
                rejected.join(", ")
            );
            process::run_inherit_env(argv, &[COOLDOWN_OFF], CHECK_TIMEOUT)
                .map_err(|error| format!("({})", first_line(&format!("{error:#}"))))
        }
    }
}

/// The first line of `text`, or nothing.
fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or_default()
}

/// The setting that lets cargo resolve a release younger than the cooldown.
const COOLDOWN_OFF: (&str, &str) = ("CARGO_RESOLVER_INCOMPATIBLE_PUBLISH_AGE", "allow");

/// Whether a second run without the cooldown may decide the check.
#[derive(Debug, PartialEq, Eq)]
enum Retry {
    /// Every crate the cooldown rejected has a trusted owner.
    Allow,
    /// The check stays failed, for this reason.
    NoRetry(String),
}

/// Decide whether the check may run again past the cooldown, given the crates
/// it rejected (at least one) and the trusted `logins`.
/// `owners` returns a crate's user logins, as
/// [`registry::crates_io_user_owners`] does, and none for a crate crates.io
/// does not know.
///
/// # Errors
///
/// Returns the error of the first `owners` call that fails.
fn retry_decision(
    rejected: &[String],
    logins: &[String],
    owners: impl Fn(&str) -> Result<Vec<String>>,
) -> Result<Retry> {
    let names = rejected.join(", ");
    if logins.is_empty() {
        return Ok(Retry::NoRetry(format!(
            "the publish-age cooldown rejected {names}; {TRUSTED_OWNERS} is unset, so no retry"
        )));
    }
    let mut untrusted = Vec::new();
    for name in rejected {
        // GitHub logins, which crates.io shows, ignore ASCII case.
        let trusted = owners(name)?.iter().any(|login| {
            logins
                .iter()
                .any(|wanted| wanted.eq_ignore_ascii_case(login))
        });
        if !trusted {
            untrusted.push(name.as_str());
        }
    }
    if untrusted.is_empty() {
        return Ok(Retry::Allow);
    }
    Ok(Retry::NoRetry(format!(
        "the publish-age cooldown rejected {names}; {} have no owner in {TRUSTED_OWNERS}, so no retry",
        untrusted.join(", ")
    )))
}

/// The crates whose release the publish-age cooldown rejected, from cargo's
/// error in `output`, sorted and without repeats.
///
/// Cargo 1.100 reports one as `failed to select a version for the
/// requirement` naming the crate, and a `version ... is too new (published
/// ...)` line follows within three lines. Another resolution error, which has
/// no such line, names no crate.
fn cooldown_rejections(output: &str) -> Vec<String> {
    const REQUIREMENT: &str = "failed to select a version for the requirement `";
    const TOO_NEW: &str = " is too new (published ";
    let plain = strip_ansi(output);
    let lines: Vec<&str> = plain.lines().collect();
    let mut names = BTreeSet::new();
    for (index, line) in lines.iter().enumerate() {
        let Some((_, rest)) = line.split_once(REQUIREMENT) else {
            continue;
        };
        let Some((name, _)) = rest.split_once(" = ") else {
            continue;
        };
        let too_new = lines
            .iter()
            .skip(index + 1)
            .filter(|next| !next.trim().is_empty())
            .take(3)
            .any(|next| next.contains(TOO_NEW));
        if too_new {
            names.insert(name.to_owned());
        }
    }
    names.into_iter().collect()
}

/// `text` without its ANSI escape sequences: `ESC [`, then everything up to a
/// final byte in `@` to `~`.
fn strip_ansi(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(next) = chars.next() {
        if next == '\x1b' && chars.next_if_eq(&'[').is_some() {
            for skipped in chars.by_ref() {
                if ('@'..='~').contains(&skipped) {
                    break;
                }
            }
        } else {
            plain.push(next);
        }
    }
    plain
}

fn plan(config: &Config, root: &Path, fragments: &[Fragment]) -> Result<Vec<Planned>> {
    let planned = publish::plan(config, root)?;
    let mut crates = Vec::new();
    for (spec, entry) in config.packages.iter().zip(&planned.packages) {
        if entry.argv.is_empty() {
            continue;
        }
        let bump = pending_bump(fragments, &entry.name);
        let pending_mark = marked_breaking(fragments, &entry.name);
        let changelog = read_changelog(&spec.changelog_path(root))?;
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
            let breaking =
                pending_mark || released_mark(&changelog, &entry.name, &version, baseline.as_ref());
            let skip = skip_reason(name, bump, breaking, &version, baseline.as_ref())?;
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

/// Whether the package in `manifest` has a library target, as Cargo resolves
/// it: an explicit `[lib]` table, or `src/lib.rs` found by
/// [target auto-discovery] unless `package.autolib` is `false`.
///
/// [target auto-discovery]: https://doc.rust-lang.org/cargo/reference/cargo-targets.html#target-auto-discovery
fn has_library(manifest: &Path) -> Result<bool> {
    let raw = fs::read_to_string(manifest).with_context(|| manifest.display().to_string())?;
    let document = raw
        .parse::<DocumentMut>()
        .with_context(|| manifest.display().to_string())?;
    if document.contains_key("lib") {
        return Ok(true);
    }
    let autolib = document
        .get("package")
        .and_then(|package| package.get("autolib"))
        .and_then(toml_edit::Item::as_bool)
        .unwrap_or(true);
    let dir = manifest.parent().unwrap_or_else(|| Path::new("."));
    Ok(autolib && dir.join("src").join("lib.rs").is_file())
}

/// Whether cargo-semver-checks can check the crate in `manifest`: it has a
/// library target, as [`has_library`] resolves it, that is not a
/// proc-macro. cargo-semver-checks 0.51.0 leaves out a proc-macro
/// (`is_lib_like_checkable_target`) and refuses to run with nothing left.
fn checkable(manifest: &Path) -> Result<bool> {
    if !has_library(manifest)? {
        return Ok(false);
    }
    let raw = fs::read_to_string(manifest).with_context(|| manifest.display().to_string())?;
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

/// The package CHANGELOG at `path`, or nothing when the file does not exist.
fn read_changelog(path: &Path) -> Result<String> {
    match fs::read_to_string(path) {
        Ok(body) => Ok(body),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error).with_context(|| path.display().to_string()),
    }
}

/// Whether the Version PR already wrote a `patch!` release: `version` is above
/// `baseline` in the same minor, and the CHANGELOG section of some version `v`
/// with `baseline < v <= version` holds the Breaking heading.
/// The baseline is the last stable release, so the sections of a pre-release
/// train above it are all in the compared delta, and each one's mark counts.
/// The Version PR consumed the fragments, so the sections are where the mark is
/// left until a stable release at or above the train reaches crates.io and
/// becomes the baseline.
fn released_mark(
    changelog: &str,
    package: &str,
    version: &Version,
    baseline: Option<&Version>,
) -> bool {
    let heading = format!("### {}", release::BREAKING);
    let Some(baseline) = baseline else {
        return false;
    };
    if (version.major, version.minor) != (baseline.major, baseline.minor) {
        return false;
    }
    release::changelog_versions(changelog, package)
        .into_iter()
        .filter(|listed| {
            Version::parse(listed).is_ok_and(|listed| listed > *baseline && listed <= *version)
        })
        .filter_map(|listed| release::changelog_section_for(changelog, package, listed))
        .any(|section| section.lines().any(|line| line.trim() == heading))
}

/// Whether a fragment in `fragments` bumps `package` by `patch!`.
fn marked_breaking(fragments: &[Fragment], package: &str) -> bool {
    fragments
        .iter()
        .flat_map(|fragment| &fragment.packages)
        .any(|named| named.name == package && named.breaking)
}

/// Why a crate at `version` whose package has `bump` pending, marked `patch!`
/// when `breaking`, is not checked against `baseline`, or `None` when it is
/// checked.
fn skip_reason(
    name: &str,
    bump: Bump,
    breaking: bool,
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
    if breaking {
        return Ok(Some(Decision::BreakingPatch));
    }
    if (baseline.major, baseline.minor) != (version.major, version.minor) {
        return Ok(Some(Decision::VersionMoved));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use anyhow::{Context, Result};
    use indoc::indoc;
    use semver::Version;
    use tempfile::TempDir;

    use super::{
        Decision, Retry, checkable, cooldown_rejections, has_library, marked_breaking,
        pending_bump, released_mark, retry_decision, skip_reason,
    };
    use crate::fragment::{Bump, Fragment, PackageBump};

    fn version(raw: &str) -> Version {
        Version::parse(raw).unwrap()
    }

    fn skip(bump: Bump, current: &str, baseline: Option<&str>) -> Option<Decision> {
        skip_marked(bump, false, current, baseline)
    }

    fn skip_marked(
        bump: Bump,
        breaking: bool,
        current: &str,
        baseline: Option<&str>,
    ) -> Option<Decision> {
        skip_reason(
            "demo",
            bump,
            breaking,
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

    /// The mark lasts as long as its fragment: once the Version PR consumed
    /// it, the next patch is checked again.
    #[test]
    fn a_patch_bang_fragment_skips_the_check_for_its_release_only() {
        assert_eq!(
            skip_marked(Bump::Patch, true, "0.8.0", Some("0.8.0")),
            Some(Decision::BreakingPatch)
        );
        assert_eq!(
            skip_marked(Bump::Patch, false, "0.8.1", Some("0.8.1")),
            None
        );
    }

    /// On the Version PR the fragment is gone, the manifest is one patch above
    /// crates.io, and the new section carries the Breaking heading. Once
    /// crates.io holds that version, the same section does not count.
    #[test]
    fn the_version_pr_finds_the_mark_in_its_changelog_section() {
        let changelog = indoc! {"
            # Changelog

            ## verctl 0.8.1

            ### Breaking

            - Replace a public field.

            ## verctl 0.8.0

            - Earlier.
        "};
        let v081 = version("0.8.1");
        assert!(released_mark(
            changelog,
            "verctl",
            &v081,
            Some(&version("0.8.0"))
        ));
        assert!(!released_mark(changelog, "verctl", &v081, Some(&v081)));
        assert!(!released_mark(changelog, "verctl", &v081, None));
        let minor = indoc! {"
            ## verctl 0.9.0

            ### Breaking

            - Replace a public field.
        "};
        assert!(!released_mark(
            minor,
            "verctl",
            &version("0.9.0"),
            Some(&version("0.8.0"))
        ));
        assert!(!released_mark(
            changelog,
            "verctl",
            &version("0.8.0"),
            Some(&version("0.7.4"))
        ));
    }

    /// The baseline is the last stable release, so the Breaking sections of
    /// the pre-release train above it are in the compared delta: the mark
    /// holds for the train's last release, and ends at the baseline.
    #[test]
    fn the_mark_of_an_earlier_release_in_the_train_counts() {
        let changelog = indoc! {"
            # Changelog

            ## verctl 0.8.6-rc.5

            - Fixed.

            ## verctl 0.8.6-rc.3

            - Changed.

            ## verctl 0.8.6-rc.2

            ### Breaking

            - Replace a public field.

            ## verctl 0.8.6-rc.1

            - First.

            ## verctl 0.8.5

            ### Breaking

            - Released with the baseline.

            ## verctl 0.8.4

            ### Breaking

            - Earlier still.
        "};
        let rc5 = version("0.8.6-rc.5");
        assert!(released_mark(
            changelog,
            "verctl",
            &rc5,
            Some(&version("0.8.5"))
        ));
        assert!(!released_mark(
            changelog,
            "verctl",
            &version("0.8.6-rc.1"),
            Some(&version("0.8.5"))
        ));
        let below = indoc! {"
            ## verctl 0.8.6-rc.5

            - Fixed.

            ## verctl 0.8.5

            ### Breaking

            - Released with the baseline.

            ## verctl 0.8.4

            ### Breaking

            - Earlier still.
        "};
        assert!(!released_mark(
            below,
            "verctl",
            &rc5,
            Some(&version("0.8.5"))
        ));
        assert!(!released_mark(
            changelog,
            "verctl",
            &rc5,
            Some(&version("0.7.9"))
        ));
    }

    #[test]
    fn a_minor_beside_the_mark_keeps_its_own_decision() {
        assert_eq!(
            skip_marked(Bump::Minor, true, "0.8.0", Some("0.8.0")),
            Some(Decision::MinorPending)
        );
    }

    #[test]
    fn the_mark_counts_only_for_the_package_it_names() {
        let fragment = Fragment {
            path: "f.md".into(),
            packages: vec![
                PackageBump {
                    name: "demo-core".into(),
                    bump: Bump::Patch,
                    breaking: true,
                },
                PackageBump {
                    name: "demo-cli".into(),
                    bump: Bump::Patch,
                    breaking: false,
                },
            ],
            summary: String::new(),
            group: None,
        };
        let fragments = [fragment];
        assert!(marked_breaking(&fragments, "demo-core"));
        assert!(!marked_breaking(&fragments, "demo-cli"));
        assert!(!marked_breaking(&fragments, "other"));
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
        let error = skip_reason("demo", Bump::Patch, false, &version("1.0.0"), None).unwrap_err();
        assert!(format!("{error:#}").contains("1.0"), "{error:#}");
    }

    #[test]
    fn the_largest_bump_for_the_package_wins() {
        let fragment = |name: &str, bump: Bump| Fragment {
            path: "f.md".into(),
            packages: vec![PackageBump {
                name: name.into(),
                bump,
                breaking: false,
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

    const CTL_TEXT: &str = indoc! {"
        error: `cargo metadata` exited with an error:     Updating crates.io index
        error: failed to select a version for the requirement `ctl-text = \"=0.7.4\"`
          version 0.7.4 is too new (published 36 hours ago, minimum age 3 days)
        location searched: crates.io index
        required by package `ctl-core v0.7.4 (/home/runner/work/ctl-core/ctl-core/ctl-core)`
        help: to preserve the min-publish-age, downgrade the requirement to \"0.7.0\"
        help: to use too-new packages anyways, re-resolve with `CARGO_RESOLVER_INCOMPATIBLE_PUBLISH_AGE=allow`
    "};

    const YAMLED: &str = indoc! {"
        error: `cargo metadata` exited with an error:     Updating crates.io index
        error: failed to select a version for the requirement `yamled = \"=0.0.13\"`
          version 0.0.13 is too new (published 39 hours ago, minimum age 3 days)
        location searched: crates.io index
        required by package `ctl-core v0.7.4 (/home/runner/work/ctl-core/ctl-core/ctl-core)`
        help: to preserve the min-publish-age, downgrade the requirement to \"0.0.12\"
        help: to use too-new packages anyways, re-resolve with `CARGO_RESOLVER_INCOMPATIBLE_PUBLISH_AGE=allow`
    "};

    const QCTL: &str = indoc! {"
        error: `cargo metadata` exited with an error:     Updating crates.io index
        error: failed to select a version for the requirement `qctl = \"=0.7.0\"`
          version 0.7.0 is too new (published 3 days ago, minimum age 3 days)
        location searched: crates.io index
        required by package `qctl v0.7.1 (/home/runner/work/qctl/qctl)`
        help: to preserve the min-publish-age, downgrade the requirement to \"0.6.0\"
        help: to use too-new packages anyways, re-resolve with `CARGO_RESOLVER_INCOMPATIBLE_PUBLISH_AGE=allow`
    "};

    #[test]
    fn the_cooldown_names_the_crate_it_rejected() {
        assert_eq!(cooldown_rejections(CTL_TEXT), ["ctl-text"]);
        assert_eq!(cooldown_rejections(QCTL), ["qctl"]);
    }

    #[test]
    fn several_rejections_are_sorted_and_listed_once() {
        let output = format!("{YAMLED}{CTL_TEXT}{YAMLED}");
        assert_eq!(cooldown_rejections(&output), ["ctl-text", "yamled"]);
    }

    #[test]
    fn ansi_colour_codes_do_not_hide_a_rejection() {
        let output = indoc! {"
            \u{1b}[1m\u{1b}[31merror\u{1b}[0m: failed to select a version for the requirement `ctl-text = \"=0.7.4\"`
            \u{1b}[1m\u{1b}[33m  version 0.7.4 is too new (published 36 hours ago, minimum age 3 days)\u{1b}[0m
        "};
        assert_eq!(cooldown_rejections(output), ["ctl-text"]);
    }

    #[test]
    fn another_resolution_error_names_no_crate() {
        let output = indoc! {"
            error: failed to select a version for the requirement `demo = \"^9.0.0\"`
            candidate versions found which didn't match: 0.1.0
            location searched: crates.io index
            required by package `ctl-core v0.7.4 (/work/ctl-core)`
            help: version 0.9.0 is too new (published 1 hour ago, minimum age 3 days)
        "};
        assert_eq!(cooldown_rejections(output), Vec::<String>::new());
        assert_eq!(
            cooldown_rejections("error: could not compile `demo`"),
            Vec::<String>::new()
        );
    }

    fn owners_of(
        table: &'static [(&'static str, &'static [&'static str])],
    ) -> impl Fn(&str) -> Result<Vec<String>> {
        move |name| {
            table
                .iter()
                .find(|(crate_name, _)| *crate_name == name)
                .map(|(_, logins)| logins.iter().map(|login| (*login).to_owned()).collect())
                .context("owners lookup failed")
        }
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn a_rejection_of_trusted_crates_only_allows_a_second_run() {
        let owners = owners_of(&[
            ("ctl-text", &["any-victor"]),
            ("yamled", &["Any-Victor", "x"]),
        ]);
        assert_eq!(
            retry_decision(
                &names(&["ctl-text", "yamled"]),
                &names(&["any-victor"]),
                owners
            )
            .unwrap(),
            Retry::Allow
        );
    }

    /// The reason of a [`Retry::NoRetry`], or nothing for [`Retry::Allow`].
    fn reason(decision: Retry) -> String {
        match decision {
            Retry::NoRetry(reason) => reason,
            Retry::Allow => String::new(),
        }
    }

    #[test]
    fn one_untrusted_crate_stops_the_second_run_and_is_named() {
        let owners = owners_of(&[("ctl-text", &["any-victor"]), ("serde", &["dtolnay"])]);
        let decision = retry_decision(
            &names(&["ctl-text", "serde"]),
            &names(&["any-victor"]),
            owners,
        )
        .unwrap();
        let reason = reason(decision);
        assert!(reason.contains("rejected ctl-text, serde"), "{reason}");
        assert!(
            reason.contains("serde have no owner in CARGO_TRUSTED_OWNERS"),
            "{reason}"
        );
    }

    /// The lookup here fails, so a decision that returns means it never ran.
    #[test]
    fn no_trusted_logins_means_no_lookup_and_no_second_run() {
        let decision = retry_decision(&names(&["ctl-text"]), &[], owners_of(&[])).unwrap();
        let reason = reason(decision);
        assert!(reason.contains("CARGO_TRUSTED_OWNERS is unset"), "{reason}");
    }

    #[test]
    fn a_failed_owners_lookup_is_an_error() {
        let owners = owners_of(&[]);
        let error =
            retry_decision(&names(&["ctl-text"]), &names(&["any-victor"]), owners).unwrap_err();
        assert!(
            format!("{error:#}").contains("owners lookup failed"),
            "{error:#}"
        );
    }

    fn manifest(dir: &TempDir, body: &str) -> PathBuf {
        let path = dir.path().join("Cargo.toml");
        fs::write(&path, body).unwrap();
        path
    }

    const PLAIN: &str = indoc! {r#"
        [package]
        name = "demo"
        version = "0.4.6"
    "#};

    #[test]
    fn a_library_found_by_auto_discovery() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/lib.rs"), "").unwrap();
        assert!(has_library(&manifest(&dir, PLAIN)).unwrap());
    }

    #[test]
    fn a_binary_only_crate() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();
        assert!(!has_library(&manifest(&dir, PLAIN)).unwrap());
    }

    #[test]
    fn an_explicit_lib_table_or_autolib_off() {
        let dir = TempDir::new().unwrap();
        let explicit = manifest(
            &dir,
            indoc! {r#"
                [package]
                name = "demo"
                version = "0.4.6"

                [lib]
                path = "lib/demo.rs"
            "#},
        );
        assert!(has_library(&explicit).unwrap());
        let off = TempDir::new().unwrap();
        fs::create_dir_all(off.path().join("src")).unwrap();
        fs::write(off.path().join("src/lib.rs"), "").unwrap();
        let path = manifest(
            &off,
            indoc! {r#"
                [package]
                name = "demo"
                version = "0.4.6"
                autolib = false
            "#},
        );
        assert!(!has_library(&path).unwrap());
    }
}
