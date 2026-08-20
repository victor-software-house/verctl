//! Rewrite collocated tool pins when a package version changes.

use crate::config::{Config, PLACEHOLDER, Pin, PinPattern};
use anyhow::{Context, Result, bail};
use regex::{NoExpand, Regex, escape};
use std::fs;
use std::path::{Component, Path, PathBuf};
use toml_edit::{DocumentMut, Item, value};

/// A version, as every pin site spells one.
const VERSION: &str = r"[0-9]+(?:\.[0-9]+)*";

/// Current declared versions of every configured package.
pub fn current_versions(root: &Path, config: &Config) -> Result<Vec<(String, String)>> {
    let mut versions = Vec::new();
    for spec in &config.packages {
        let path = root.join(&spec.path);
        let driver = spec.resolve(config, root)?;
        let raw = fs::read_to_string(&path).with_context(|| path.display().to_string())?;
        versions.push((spec.name.clone(), driver.read(&raw)?.trim().to_owned()));
    }
    Ok(versions)
}

pub fn write(root: &Path, pins: &[Pin], versions: &[(String, String)]) -> Result<Vec<PathBuf>> {
    apply(root, pins, versions, true)
}

/// Same checks as `write`, without touching disk.
pub fn plan(root: &Path, pins: &[Pin], versions: &[(String, String)]) -> Result<Vec<PathBuf>> {
    apply(root, pins, versions, false)
}

fn apply(
    root: &Path,
    pins: &[Pin],
    versions: &[(String, String)],
    persist: bool,
) -> Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    for pin in pins {
        let Some((_, version)) = versions.iter().find(|(name, _)| name == &pin.package) else {
            continue;
        };
        ensure_inside(root, &pin.file)?;
        let path = root.join(&pin.file);
        let raw =
            fs::read_to_string(&path).with_context(|| format!("read pin {}", path.display()))?;
        let mut body = raw;
        if let Some(tool) = &pin.tool {
            body = rewrite_tool(&body, tool, version, &path)?;
        }
        for pattern in &pin.patterns {
            body = rewrite_pattern(&body, pattern, version)
                .with_context(|| format!("pin {}", path.display()))?;
        }
        if persist {
            fs::write(&path, body).with_context(|| format!("write pin {}", path.display()))?;
        }
        written.push(path);
    }
    Ok(written)
}

fn ensure_inside(root: &Path, file: &Path) -> Result<()> {
    if file.is_absolute()
        || file
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        bail!("pin file must stay inside the repo: {}", file.display());
    }
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let path = root.join(file);
    if let Ok(meta) = fs::symlink_metadata(&path)
        && meta.file_type().is_symlink()
    {
        bail!("pin file must not be a symlink: {}", path.display());
    }
    let resolved = path.canonicalize().unwrap_or(path);
    if !resolved.starts_with(&root) {
        bail!("pin file must stay inside the repo: {}", file.display());
    }
    Ok(())
}

/// A mise `[tools]` entry, plus every `?ref=v…` include naming its repo.
fn rewrite_tool(body: &str, tool: &str, version: &str, path: &Path) -> Result<String> {
    let mut doc: DocumentMut = body
        .parse()
        .with_context(|| format!("parse pin {}", path.display()))?;
    let tools = doc
        .get_mut("tools")
        .and_then(Item::as_table_like_mut)
        .with_context(|| format!("{} has no [tools]", path.display()))?;
    if tools.get(tool).is_none() {
        bail!("{} has no tools.{tool}", path.display());
    }
    tools.insert(tool, value(version));
    rewrite_own_refs(&doc.to_string(), tool, version)
}

/// The version in every occurrence of `pattern.match`, where `{version}`
/// marks it. The rest is literal, so a bash header, a README sentence, and a
/// YAML value all rewrite the same way with nothing assumed about the file.
///
/// How often the file must say it is declared, never inferred, and both
/// directions are release-stopping: fewer than declared means the pin no
/// longer tracks anything, more means a version spelling nobody accounted
/// for. See `Occurrences` for the vocabulary.
fn rewrite_pattern(body: &str, pattern: &PinPattern, version: &str) -> Result<String> {
    let text = &pattern.r#match;
    let (before, after) = text
        .split_once(PLACEHOLDER)
        .with_context(|| format!("pattern {text:?} has no {PLACEHOLDER}"))?;
    if after.contains(PLACEHOLDER) {
        bail!("pattern {text:?} has more than one {PLACEHOLDER}");
    }
    // `R` is CRLF mode: without it `$` sits only before `\n`, so a file checked
    // out with CRLF endings has a `\r` between the version and the line end and
    // matches nothing — a correct file stopping a release.
    let (open, close) = if pattern.whole_line {
        ("(?mR)^", "$")
    } else {
        ("", "")
    };
    let finder = Regex::new(&format!(
        "{open}{}{VERSION}{}{close}",
        escape(before),
        escape(after)
    ))
    .with_context(|| format!("pattern {text:?}"))?;
    let found = finder.find_iter(body).count();
    if !pattern.occurrences.allows(found) {
        bail!(
            "pattern {text:?} matches {found} times, expected {}",
            pattern.occurrences
        );
    }
    let pinned = format!("{before}{version}{after}");
    Ok(finder.replace_all(body, NoExpand(&pinned)).into_owned())
}

/// Every `?ref=v…` include that names this tool's own repository. A sibling
/// repo's include sitting in the same file is not ours to move, so the URL
/// path must carry the tool's own name.
fn rewrite_own_refs(body: &str, tool: &str, version: &str) -> Result<String> {
    let name = escape(tool.rsplit(['/', ':']).next().unwrap_or(tool));
    let finder = Regex::new(&format!(
        r#"(?<include>/{name}(?:\.git)?//[^\s"',\]]*\?ref=v){VERSION}"#
    ))
    .with_context(|| format!("tool {tool:?}"))?;
    Ok(finder
        .replace_all(body, format!("${{include}}{version}"))
        .into_owned())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::config::Occurrences;
    use indoc::indoc;
    use tempfile::TempDir;

    const VERCTL: &str = "github:victor-software-house/verctl";
    const DOC: &str = "README.md";

    /// One hand-authored file, every version spelling a repo really carries:
    /// a tagged include, a tarball invocation, a bare mention, a sibling
    /// repo's pin that is not ours, and a non-version that must not match.
    const BEFORE: &str = indoc! {r#"
        # verctl

        ```toml
        [task_config]
        includes = [
          "git::https://github.com/victor-software-house/verctl.git//tasks/ver?ref=v0.0.1",
        ]
        ```

        Install: mise x github:victor-software-house/verctl@0.0.1 -- verctl --version
        Pin verctl@0.0.1 in CI.
        Unrelated: qctl@0.0.1 and forkctl 0.0.19 stay put.
        Floating: verctl@latest.
    "#};

    /// What a release does to `BEFORE`: the exact bytes, or the failure.
    enum Outcome {
        Serves(&'static str),
        Fails(&'static str),
    }

    fn versions(to: &str) -> Vec<(String, String)> {
        vec![("verctl".into(), to.into())]
    }

    /// A pin on this repo's own tool entry in `file`.
    fn tool_pin(file: &str, package: &str) -> Pin {
        Pin {
            file: PathBuf::from(file),
            tool: Some(VERCTL.into()),
            pattern_ids: Vec::new(),
            patterns: Vec::new(),
            package: package.into(),
        }
    }

    /// A pin that knows nothing about `file` but the text around the version.
    fn pattern_pin(file: &str, patterns: &[(&str, Occurrences)]) -> Pin {
        Pin {
            file: PathBuf::from(file),
            tool: None,
            pattern_ids: Vec::new(),
            patterns: patterns
                .iter()
                .map(|(text, occurrences)| PinPattern {
                    r#match: (*text).to_owned(),
                    occurrences: *occurrences,
                    whole_line: false,
                })
                .collect(),
            package: "verctl".into(),
        }
    }

    fn write_file(root: &TempDir, file: &str, body: &str) -> PathBuf {
        let path = root.path().join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, body).unwrap();
        path
    }

    /// One before, many pattern sets and versions, each asserted whole.
    #[test]
    #[allow(clippy::too_many_lines)] // one case per scenario; a table, not logic
    fn one_document_pins_every_release_exactly() {
        let tarball = "github:victor-software-house/verctl@{version}";
        let include = "verctl.git//tasks/ver?ref=v{version}";
        let bare = "verctl@{version}";
        let cases = [
            (
                "every declared spelling moves together",
                &[
                    (tarball, Occurrences::Once),
                    (include, Occurrences::Once),
                    ("verctl@{version} in CI", Occurrences::Once),
                ][..],
                "0.0.2",
                Outcome::Serves(indoc! {r#"
                    # verctl

                    ```toml
                    [task_config]
                    includes = [
                      "git::https://github.com/victor-software-house/verctl.git//tasks/ver?ref=v0.0.2",
                    ]
                    ```

                    Install: mise x github:victor-software-house/verctl@0.0.2 -- verctl --version
                    Pin verctl@0.0.2 in CI.
                    Unrelated: qctl@0.0.1 and forkctl 0.0.19 stay put.
                    Floating: verctl@latest.
                "#}),
            ),
            (
                "a version with a two-digit part replaces the whole number",
                &[(tarball, Occurrences::Once)][..],
                "0.0.10",
                Outcome::Serves(indoc! {r#"
                    # verctl

                    ```toml
                    [task_config]
                    includes = [
                      "git::https://github.com/victor-software-house/verctl.git//tasks/ver?ref=v0.0.1",
                    ]
                    ```

                    Install: mise x github:victor-software-house/verctl@0.0.10 -- verctl --version
                    Pin verctl@0.0.1 in CI.
                    Unrelated: qctl@0.0.1 and forkctl 0.0.19 stay put.
                    Floating: verctl@latest.
                "#}),
            ),
            (
                "a release to the version already served changes nothing",
                &[(tarball, Occurrences::Once), (include, Occurrences::Once)][..],
                "0.0.1",
                Outcome::Serves(BEFORE),
            ),
            (
                "a spelling the file says twice, declared twice, moves twice",
                &[(bare, Occurrences::Exactly(2))][..],
                "1.2.3",
                Outcome::Serves(indoc! {r#"
                    # verctl

                    ```toml
                    [task_config]
                    includes = [
                      "git::https://github.com/victor-software-house/verctl.git//tasks/ver?ref=v0.0.1",
                    ]
                    ```

                    Install: mise x github:victor-software-house/verctl@1.2.3 -- verctl --version
                    Pin verctl@1.2.3 in CI.
                    Unrelated: qctl@0.0.1 and forkctl 0.0.19 stay put.
                    Floating: verctl@latest.
                "#}),
            ),
            (
                "a spelling whose mentions come and go, declared as many",
                &[(bare, Occurrences::Many)][..],
                "1.2.3",
                Outcome::Serves(indoc! {r#"
                    # verctl

                    ```toml
                    [task_config]
                    includes = [
                      "git::https://github.com/victor-software-house/verctl.git//tasks/ver?ref=v0.0.1",
                    ]
                    ```

                    Install: mise x github:victor-software-house/verctl@1.2.3 -- verctl --version
                    Pin verctl@1.2.3 in CI.
                    Unrelated: qctl@0.0.1 and forkctl 0.0.19 stay put.
                    Floating: verctl@latest.
                "#}),
            ),
            (
                "two mentions clear a floor of two",
                &[(bare, Occurrences::AtLeast(2))][..],
                "0.0.2",
                Outcome::Serves(indoc! {r#"
                    # verctl

                    ```toml
                    [task_config]
                    includes = [
                      "git::https://github.com/victor-software-house/verctl.git//tasks/ver?ref=v0.0.1",
                    ]
                    ```

                    Install: mise x github:victor-software-house/verctl@0.0.2 -- verctl --version
                    Pin verctl@0.0.2 in CI.
                    Unrelated: qctl@0.0.1 and forkctl 0.0.19 stay put.
                    Floating: verctl@latest.
                "#}),
            ),
            (
                "the same spelling left at the default arity fails",
                &[(bare, Occurrences::Once)][..],
                "1.2.3",
                Outcome::Fails("matches 2 times, expected exactly once"),
            ),
            (
                "a spelling declared more often than the file says it fails",
                &[(tarball, Occurrences::Exactly(2))][..],
                "0.0.2",
                Outcome::Fails("matches 1 times, expected exactly 2 times"),
            ),
            (
                "one mention does not clear a floor of two",
                &[(tarball, Occurrences::AtLeast(2))][..],
                "0.0.2",
                Outcome::Fails("matches 1 times, expected 2 or more times"),
            ),
            (
                "a spelling the file lost fails",
                &[("verctl@{version} in production", Occurrences::Once)][..],
                "0.0.2",
                Outcome::Fails("matches 0 times, expected exactly once"),
            ),
            (
                "a spelling declared as many, with none left, fails",
                &[("verctl@{version} in production", Occurrences::Many)][..],
                "0.0.2",
                Outcome::Fails("matches 0 times, expected one or more"),
            ),
            (
                "a retired spelling that came back fails",
                &[(tarball, Occurrences::Never)][..],
                "0.0.2",
                Outcome::Fails("matches 1 times, expected never"),
            ),
            (
                "a retired spelling that stayed gone is satisfied",
                &[("verctl@{version} in production", Occurrences::Never)][..],
                "0.0.2",
                Outcome::Serves(BEFORE),
            ),
            (
                "a pattern with no placeholder fails",
                &[("verctl@0.0.1", Occurrences::Once)][..],
                "0.0.2",
                Outcome::Fails("has no {version}"),
            ),
            (
                "a pattern with two placeholders fails",
                &[("verctl@{version}-{version}", Occurrences::Once)][..],
                "0.0.2",
                Outcome::Fails("has more than one {version}"),
            ),
            (
                "a floating pin is not a version and cannot be tracked",
                &[("verctl@{version}.\n", Occurrences::Once)][..],
                "0.0.2",
                Outcome::Fails("matches 0 times, expected exactly once"),
            ),
        ];
        for (label, patterns, version, outcome) in cases {
            let root = TempDir::new().unwrap();
            let path = write_file(&root, DOC, BEFORE);
            let result = write(
                root.path(),
                &[pattern_pin(DOC, patterns)],
                &versions(version),
            );
            match outcome {
                Outcome::Serves(after) => {
                    assert_eq!(result.unwrap(), [path.clone()][..], "{label}");
                    assert_eq!(fs::read_to_string(&path).unwrap(), after, "{label}");
                }
                Outcome::Fails(complaint) => {
                    let report = format!("{:#}", result.unwrap_err());
                    assert!(report.contains(complaint), "{label}: {report}");
                    assert!(report.contains(DOC), "{label}: {report}");
                    assert_eq!(fs::read_to_string(&path).unwrap(), BEFORE, "{label}");
                }
            }
        }
    }

    /// A prerelease is not a version this rewriter tracks. It must fail the
    /// release, never rewrite the numeric head and drop the tail.
    #[test]
    fn a_prerelease_pin_fails_instead_of_half_rewriting() {
        let root = TempDir::new().unwrap();
        let before = "Install verctl@1.0.0-rc.1 today.\n";
        let path = write_file(&root, DOC, before);
        let pins = [pattern_pin(
            DOC,
            &[("verctl@{version} today", Occurrences::Once)],
        )];
        let err = write(root.path(), &pins, &versions("1.0.0")).unwrap_err();
        assert!(format!("{err:#}").contains("matches 0 times"), "{err:#}");
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
    }

    /// Two pins in the same file, back to back, with nothing between them.
    #[test]
    fn adjacent_matches_both_move() {
        let root = TempDir::new().unwrap();
        let path = write_file(&root, DOC, "verctl@0.0.1verctl@0.0.1\n");
        let pins = [pattern_pin(
            DOC,
            &[("verctl@{version}", Occurrences::Exactly(2))],
        )];
        write(root.path(), &pins, &versions("0.0.2")).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "verctl@0.0.2verctl@0.0.2\n"
        );
    }

    /// `$1` is a valid capture reference, so an expanding replacement would
    /// swallow it. A served task file passes `"$1"` right beside its pin.
    #[test]
    fn a_dollar_sign_in_the_pattern_stays_a_dollar_sign() {
        let root = TempDir::new().unwrap();
        let before = "exec verctl@0.0.1 \"$1\"\n";
        let path = write_file(&root, "tasks/ver/ver", before);
        let pins = [pattern_pin(
            "tasks/ver/ver",
            &[("verctl@{version} \"$1\"", Occurrences::Once)],
        )];
        write(root.path(), &pins, &versions("0.0.2")).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "exec verctl@0.0.2 \"$1\"\n"
        );
    }

    /// The text around the version is literal: `.`, `?`, and `//` mean
    /// themselves, so a line spelled with other characters is not a match.
    #[test]
    fn the_text_around_the_version_is_literal_not_a_regex() {
        let root = TempDir::new().unwrap();
        let before = indoc! {"
            verctl.git//tasks/ver?ref=v0.0.1
            verctlxgitZZtasksZverYrefZv0.0.1
        "};
        let path = write_file(&root, DOC, before);
        let pins = [pattern_pin(
            DOC,
            &[("verctl.git//tasks/ver?ref=v{version}", Occurrences::Once)],
        )];
        write(root.path(), &pins, &versions("0.0.2")).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            indoc! {"
                verctl.git//tasks/ver?ref=v0.0.2
                verctlxgitZZtasksZverYrefZv0.0.1
            "}
        );
    }

    /// A file that parses as nothing: the served task is a bash script.
    #[test]
    fn rewrites_a_pin_in_a_file_that_is_not_toml() {
        let root = TempDir::new().unwrap();
        let before = indoc! {r#"
            #!/usr/bin/env bash
            #MISE tools = { "github:victor-software-house/verctl" = "0.0.1" }

            exec verctl "$@"
        "#};
        let path = write_file(&root, "tasks/ver/ver", before);
        let pins = [pattern_pin(
            "tasks/ver/ver",
            &[(
                r#""github:victor-software-house/verctl" = "{version}""#,
                Occurrences::Once,
            )],
        )];
        write(root.path(), &pins, &versions("0.0.2")).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            indoc! {r#"
                #!/usr/bin/env bash
                #MISE tools = { "github:victor-software-house/verctl" = "0.0.2" }

                exec verctl "$@"
            "#}
        );
    }

    #[test]
    fn a_pattern_pin_on_a_package_no_fragment_bumped_stays_put() {
        let root = TempDir::new().unwrap();
        let path = write_file(&root, DOC, BEFORE);
        let mut pin = pattern_pin(DOC, &[("verctl@{version} in CI", Occurrences::Once)]);
        pin.package = "other".into();
        assert_eq!(
            write(root.path(), &[pin], &versions("0.0.2")).unwrap(),
            Vec::<PathBuf>::new()
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), BEFORE);
    }

    #[test]
    fn a_dry_run_reports_the_file_without_writing_it() {
        let root = TempDir::new().unwrap();
        let path = write_file(&root, DOC, BEFORE);
        let pins = [pattern_pin(
            DOC,
            &[("verctl@{version} in CI", Occurrences::Once)],
        )];
        assert_eq!(
            plan(root.path(), &pins, &versions("0.0.2")).unwrap(),
            [path.clone()][..]
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), BEFORE);
    }

    #[test]
    fn updates_the_named_tool_and_keeps_siblings() {
        let root = TempDir::new().unwrap();
        let path = write_file(
            &root,
            "mise.release.toml",
            indoc! {r#"
                [tools]
                leftover = "1"
                "github:victor-software-house/verctl" = "0.0.1"
            "#},
        );
        let pins = [tool_pin("mise.release.toml", "verctl")];
        write(root.path(), &pins, &versions("0.0.2")).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            indoc! {r#"
                [tools]
                leftover = "1"
                "github:victor-software-house/verctl" = "0.0.2"
            "#}
        );
    }

    #[test]
    fn skips_when_the_package_is_not_in_the_plan() {
        let root = TempDir::new().unwrap();
        let before = indoc! {r#"
            [tools]
            "github:victor-software-house/verctl" = "0.0.1"
        "#};
        let path = write_file(&root, "mise.release.toml", before);
        let pins = [tool_pin("mise.release.toml", "other")];
        assert_eq!(
            write(root.path(), &pins, &versions("0.0.2")).unwrap(),
            Vec::<PathBuf>::new()
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
    }

    /// A tool entry and the includes that fetch it, in one file, at once —
    /// including a sibling repo's include, which is not ours to move.
    #[test]
    fn a_tool_entry_and_its_own_includes_move_together() {
        let root = TempDir::new().unwrap();
        let path = write_file(
            &root,
            "mise.toml",
            indoc! {r#"
                [tools]
                "github:victor-software-house/verctl" = "0.0.1"

                [task_config]
                includes = [
                  "git::https://github.com/victor-software-house/verctl.git//tasks/ver?ref=v1.2.3",
                  "git::https://github.com/victor-software-house/qctl.git//tasks/q?ref=v0.0.1",
                ]
            "#},
        );
        let pins = [tool_pin("mise.toml", "verctl")];
        write(root.path(), &pins, &versions("0.0.2")).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            indoc! {r#"
                [tools]
                "github:victor-software-house/verctl" = "0.0.2"

                [task_config]
                includes = [
                  "git::https://github.com/victor-software-house/verctl.git//tasks/ver?ref=v0.0.2",
                  "git::https://github.com/victor-software-house/qctl.git//tasks/q?ref=v0.0.1",
                ]
            "#}
        );
    }

    /// A TOML array with no space after the comma: only the quote and comma
    /// separate our include from the next repo's.
    #[test]
    fn one_include_ends_where_the_next_begins() {
        let root = TempDir::new().unwrap();
        let path = write_file(
            &root,
            "mise.toml",
            indoc! {r#"
                [tools]
                "github:victor-software-house/verctl" = "0.0.1"
                [task_config]
                includes = ["git::https://example.com/verctl.git//a?ref=v0.0.1","git::https://example.com/qctl.git//b?ref=v0.0.1"]
            "#},
        );
        let pins = [tool_pin("mise.toml", "verctl")];
        write(root.path(), &pins, &versions("0.0.2")).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            indoc! {r#"
                [tools]
                "github:victor-software-house/verctl" = "0.0.2"
                [task_config]
                includes = ["git::https://example.com/verctl.git//a?ref=v0.0.2","git::https://example.com/qctl.git//b?ref=v0.0.1"]
            "#}
        );
    }

    /// A line-owning spelling anchors on the line, not on whatever text sits
    /// next to it. Every case here is the same pattern against a different
    /// document, because what changes is the neighbourhood, not the spelling.
    #[test]
    fn a_whole_line_pattern_owns_its_line_and_nothing_else() {
        let cases = [
            (
                "the line moves, and the key after it is irrelevant",
                indoc! {"
                    ---
                    name: demo
                    version: 0.0.1
                    files: [a.md]
                    ---
                "},
                Ok(indoc! {"
                    ---
                    name: demo
                    version: 0.0.2
                    files: [a.md]
                    ---
                "}),
            ),
            (
                "the same words inside a sentence are not the line",
                indoc! {"
                    ---
                    version: 0.0.1
                    ---

                    Write version: 9.9.9 when you mean the tool's own.
                "},
                Ok(indoc! {"
                    ---
                    version: 0.0.2
                    ---

                    Write version: 9.9.9 when you mean the tool's own.
                "}),
            ),
            (
                "a tail the version does not cover leaves the line unmatched",
                indoc! {"
                    ---
                    version: 0.0.1-rc1
                    ---
                "},
                Err("matches 0 times"),
            ),
            (
                "a line ends where the file says it does, CRLF included",
                "---\r\nversion: 0.0.1\r\n---\r\n",
                Ok("---\r\nversion: 0.0.2\r\n---\r\n"),
            ),
            (
                "an indented key is a line that starts with something else",
                indoc! {"
                    ---
                    tool:
                      version: 0.0.1
                    ---
                "},
                Err("matches 0 times"),
            ),
        ];
        for (scenario, before, expected) in cases {
            let root = TempDir::new().unwrap();
            let path = write_file(&root, "SKILL.md", before);
            let pins = [Pin {
                file: PathBuf::from("SKILL.md"),
                tool: None,
                pattern_ids: Vec::new(),
                patterns: vec![PinPattern {
                    r#match: "version: {version}".into(),
                    occurrences: Occurrences::Once,
                    whole_line: true,
                }],
                package: "verctl".into(),
            }];
            match (write(root.path(), &pins, &versions("0.0.2")), expected) {
                (Ok(_), Ok(after)) => {
                    assert_eq!(fs::read_to_string(&path).unwrap(), after, "{scenario}");
                }
                (Err(error), Err(needle)) => {
                    let text = format!("{error:#}");
                    assert!(text.contains(needle), "{scenario}: {text}");
                    assert_eq!(
                        fs::read_to_string(&path).unwrap(),
                        before,
                        "{scenario}: nothing is written when the pin cannot be rewritten"
                    );
                }
                (outcome, _) => panic!("{scenario}: {outcome:?}"),
            }
        }
    }

    /// Owning a line says nothing about how many lines: the arity still does.
    #[test]
    fn a_whole_line_pattern_counts_lines_the_way_its_arity_says() {
        let root = TempDir::new().unwrap();
        let before = indoc! {"
            version: 0.0.1
            not the line: version: 0.0.1
            version: 0.0.1
        "};
        let path = write_file(&root, "pinned.md", before);
        let two = |occurrences| {
            [Pin {
                file: PathBuf::from("pinned.md"),
                tool: None,
                pattern_ids: Vec::new(),
                patterns: vec![PinPattern {
                    r#match: "version: {version}".into(),
                    occurrences,
                    whole_line: true,
                }],
                package: "verctl".into(),
            }]
        };
        let err = write(root.path(), &two(Occurrences::Once), &versions("0.0.2")).unwrap_err();
        assert!(format!("{err:#}").contains("matches 2 times"), "{err:#}");
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            before,
            "an arity the file breaks writes nothing"
        );

        write(
            root.path(),
            &two(Occurrences::Exactly(2)),
            &versions("0.0.2"),
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            indoc! {"
                version: 0.0.2
                not the line: version: 0.0.1
                version: 0.0.2
            "},
            "both lines move and the one that only contains the words does not"
        );
    }

    /// A tool and a pattern in one pin: the structural rewrite for the table
    /// mise owns, the pattern for the prose beside it.
    #[test]
    fn a_tool_and_a_pattern_in_one_pin_both_apply() {
        let root = TempDir::new().unwrap();
        let path = write_file(
            &root,
            "mise.toml",
            indoc! {r#"
                [tools]
                "github:victor-software-house/verctl" = "0.0.1"

                # Consumers run verctl@0.0.1.
            "#},
        );
        let pins = [Pin {
            file: PathBuf::from("mise.toml"),
            tool: Some(VERCTL.into()),
            pattern_ids: Vec::new(),
            patterns: vec![PinPattern {
                r#match: "verctl@{version}".into(),
                occurrences: Occurrences::Once,
                whole_line: false,
            }],
            package: "verctl".into(),
        }];
        write(root.path(), &pins, &versions("0.0.2")).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            indoc! {r#"
                [tools]
                "github:victor-software-house/verctl" = "0.0.2"

                # Consumers run verctl@0.0.2.
            "#}
        );
    }

    #[test]
    fn a_tool_pin_needs_the_table_it_names() {
        let root = TempDir::new().unwrap();
        let before = "[tools]\nleftover = \"1\"\n";
        let path = write_file(&root, "mise.release.toml", before);
        let err = write(
            root.path(),
            &[tool_pin("mise.release.toml", "verctl")],
            &versions("0.0.2"),
        )
        .unwrap_err();
        assert!(format!("{err:#}").contains("has no tools."), "{err:#}");
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
    }

    #[test]
    fn rejects_parent_dir_pins() {
        let err = ensure_inside(Path::new("/tmp"), Path::new("../secret")).unwrap_err();
        assert!(format!("{err:#}").contains("inside the repo"), "{err:#}");
    }

    #[test]
    fn rejects_symlink_pins() {
        let root = TempDir::new().unwrap();
        let target = root.path().join("outside.txt");
        fs::write(&target, "x\n").unwrap();
        let link = root.path().join("mise.release.toml");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let err = ensure_inside(root.path(), Path::new("mise.release.toml")).unwrap_err();
        assert!(format!("{err:#}").contains("symlink"), "{err:#}");
    }
}
