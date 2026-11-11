//! Render the files a repo serves, from templates that say what they are for.
//!
//! Every template lives flat in `.ctl/templates/` and **declares where it
//! goes** in Jinja's own syntax. It needs no manifest beside it and no mirrored
//! directory tree:
//!
//! ```jinja
//! {%- set path = "tasks/q" -%}
//! {%- set executable = true -%}
//! #!/usr/bin/env bash
//! #MISE tools = { "github:victor-software-house/qctl" = "{{ versions["qctl"] }}" }
//! ```
//!
//! Jinja has no frontmatter, but a top-level `{% set %}` is its equivalent.
//! It is an export, readable from the evaluated state, so the metadata
//! travels inside the file it describes. `q.jinja` above serves `tasks/q/q`.
//! The file name defaults to the template's own. Most templates therefore
//! declare one line, and nothing accumulates in configuration.
//!
//! `Declared` is the whole schema, parsed once and resolved into a checked
//! `Served`. Trim declarations with `{%- … -%}`: a shebang has to stay on line
//! one.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use ctl_core::Surface;
use ctl_core::surface::add_fragments;
use ctl_core::validate::{inside_the_repo, one_file_name};
use garde::Validate;
use minijinja::value::Value;
use minijinja::{Environment, State, UndefinedBehavior, context};
use serde::Deserialize;

use crate::cli::Cli;
use crate::config::{Pin, Templates};
use crate::git;

/// What a template declares about the file it serves. The declarations are
/// top-level `{%- set … -%}` exports in Jinja's own syntax, read as one schema
/// and checked at the boundary. Other exports are the template's own business.
/// A name it uses to build a line is not a declaration.
#[derive(Debug, Deserialize, Validate)]
#[garde(allow_unvalidated)]
struct Declared {
    /// The directory the target goes in. When omitted, the repository root.
    #[serde(default)]
    #[garde(custom(inside_the_repo))]
    path: PathBuf,
    /// The target file's name. When undeclared, it is this template's own name
    /// without the suffix, which a served file almost always wants. The
    /// fallback is seeded before parsing, so this is never absent.
    #[garde(custom(one_file_name))]
    name: String,
    /// Whether the target is executable.
    ///
    /// Git records one bit of mode for a
    /// regular file and nothing else ("only 0755 and 0644 are valid for
    /// regular files", gitformat-index). Its other modes are not files a
    /// template renders: 120000 is a symlink, 160000 a gitlink, 040000 a tree.
    /// So there is no third state, and no mode field: when omitted, the file is
    /// 0644.
    #[serde(default)]
    executable: bool,
}

/// Render every served template and return the files written.
///
/// # Errors
///
/// Returns an error when:
///
/// - the tracked files cannot be listed;
/// - a template exists in the source tree but is neither tracked nor ignored;
/// - a template cannot be read, parsed, or rendered;
/// - a template's declarations do not match the schema;
/// - a template's target escapes the repository or is a symlink;
/// - a target cannot be written.
pub fn write(
    root: &Path,
    config: &Templates,
    pins: &[Pin],
    versions: &[(String, String)],
) -> Result<Vec<PathBuf>> {
    let rendered = render(root, config, pins, versions)?;
    for served in &rendered {
        let path = root.join(&served.target);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
        fs::write(&path, &served.body)
            .with_context(|| format!("write {}", served.target.display()))?;
        let mode = if served.executable { 0o755 } else { 0o644 };
        fs::set_permissions(&path, fs::Permissions::from_mode(mode))
            .with_context(|| format!("chmod {}", served.target.display()))?;
    }
    Ok(rendered
        .into_iter()
        .map(|served| root.join(served.target))
        .collect())
}

/// Whether this repo serves anything from a template at all.
///
/// A repo that serves nothing must not pay for what rendering needs. That cost
/// is reading every package's manifest to build the version map, so the caller
/// checks first.
///
/// # Errors
///
/// Returns an error when the tracked files cannot be listed, or when a
/// template exists in the source tree but is neither tracked nor ignored.
pub fn any(root: &Path, config: &Templates) -> Result<bool> {
    Ok(!sources(root, config)?.is_empty())
}

/// Same checks as `write`, without touching disk.
///
/// # Errors
///
/// Returns an error under the conditions of [`write()`], except that no target
/// is written, so a write failure cannot occur.
pub fn plan(
    root: &Path,
    config: &Templates,
    pins: &[Pin],
    versions: &[(String, String)],
) -> Result<Vec<PathBuf>> {
    Ok(render(root, config, pins, versions)?
        .into_iter()
        .map(|served| root.join(served.target))
        .collect())
}

/// A served file whose bytes differ from what its template renders.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Drift {
    /// The served file, relative to the repository root.
    pub target: PathBuf,
    /// The template that renders it.
    pub template: PathBuf,
}

/// Every served file that differs from its rendering at `versions`, missing
/// files included, so one run lists every file to fix.
///
/// # Errors
///
/// Returns an error under the conditions of [`plan()`], or when a target
/// exists and cannot be read.
pub fn drift(
    root: &Path,
    config: &Templates,
    pins: &[Pin],
    versions: &[(String, String)],
) -> Result<Vec<Drift>> {
    let mut drifted = Vec::new();
    for served in render(root, config, pins, versions)? {
        let path = root.join(&served.target);
        let matches = match fs::read(&path) {
            Ok(bytes) => bytes == served.body.as_bytes(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => {
                return Err(error).with_context(|| format!("read {}", served.target.display()));
            }
        };
        if !matches {
            drifted.push(Drift {
                target: served.target,
                template: served.template,
            });
        }
    }
    Ok(drifted)
}

/// One template's output, held in memory until every template has rendered
/// and no target is claimed twice.
struct Served {
    template: PathBuf,
    target: PathBuf,
    body: String,
    executable: bool,
}

/// Render every template in memory. Nothing is written here, so a template
/// that fails, or a target a `pins` entry also rewrites, stops the run before
/// any file changes.
fn render(
    root: &Path,
    config: &Templates,
    pins: &[Pin],
    versions: &[(String, String)],
) -> Result<Vec<Served>> {
    let verctl_surface = Surface::new::<Cli>("ver");
    let context = context! {
        versions => versions
            .iter()
            .map(|(name, version)| (name.clone(), version.clone()))
            .collect::<BTreeMap<_, _>>(),
        verctl_surface,
    };
    let mut environment = Environment::new();
    environment.set_undefined_behavior(UndefinedBehavior::Strict);
    environment.set_keep_trailing_newline(true);
    add_fragments(&mut environment).context("register ctl-core operator fragments")?;

    let mut rendered = Vec::new();
    for template in sources(root, config)? {
        let source = fs::read_to_string(root.join(&template))
            .with_context(|| format!("read template {}", template.display()))?;
        let name = template.display().to_string();
        let compiled = environment
            .template_from_named_str(&name, &source)
            .with_context(|| format!("parse template {name}"))?;
        let captured = compiled
            .render_captured(&context)
            .with_context(|| format!("render template {}", template.display()))?;
        let body = captured.output().to_owned();
        let declared = Declared::parse(&template, config, captured.state())?;
        let target = declared.target();
        ensure_inside(root, &target).with_context(|| format!("template {}", template.display()))?;
        refuse_pinned(&template, &target, pins)?;
        rendered.push(Served {
            template,
            target,
            body,
            executable: declared.executable,
        });
    }
    Ok(rendered)
}

/// A served file has one owner. `version` rewrites pins before it renders
/// templates. A template would then overwrite a pin's edit without a word. The
/// target is known only once its template has rendered.
fn refuse_pinned(template: &Path, target: &Path, pins: &[Pin]) -> Result<()> {
    if let Some(pin) = pins
        .iter()
        .find(|pin| without_cur_dir(&pin.file).eq(without_cur_dir(target)))
    {
        bail!(
            "{target} has two owners: template {template} renders it, and the pins entry for {file} (package {package}) rewrites it; keep one",
            target = target.display(),
            template = template.display(),
            file = pin.file.display(),
            package = pin.package,
        );
    }
    Ok(())
}

/// A path's parts without `.`. `Path::components()` keeps a leading `.`, so
/// `./tools.toml` and `tools.toml` would compare unequal while naming one file.
/// Both paths are already refused when they climb with `..`, and either file
/// may not exist yet, so dropping `.` is all the comparison needs.
fn without_cur_dir(path: &Path) -> impl Iterator<Item = Component<'_>> {
    path.components()
        .filter(|part| !matches!(part, Component::CurDir))
}

impl Declared {
    /// Read the whole schema at once from an evaluated template, with `name`
    /// seeded. Types and shapes belong to the schema: `path = 5` and
    /// `name = "a/b"` fail here instead of at write time.
    fn parse(template: &Path, config: &Templates, state: &State<'_, '_>) -> Result<Self> {
        let mut exports: BTreeMap<&str, Value> = state
            .exports()
            .into_iter()
            .filter_map(|name| state.lookup(name).map(|value| (name, value)))
            .filter(|(_, value)| !value.is_undefined() && !value.is_none())
            .collect();
        let own_name = template
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(&config.suffix))
            .with_context(|| {
                format!(
                    "{}: template name does not end in {}",
                    template.display(),
                    config.suffix
                )
            })?;
        exports
            .entry("name")
            .or_insert_with(|| Value::from(own_name));
        let declared = Self::deserialize(Value::from_serialize(&exports))
            .with_context(|| format!("{}: declarations", template.display()))?;
        declared
            .validate()
            .with_context(|| format!("{}: declarations", template.display()))?;
        Ok(declared)
    }

    /// Where the output goes.
    fn target(&self) -> PathBuf {
        self.path.join(&self.name)
    }
}

/// Every tracked template in the source tree. Flat or nested, the layout does
/// not matter: each template declares its own target.
///
/// Only tracked templates count, because `version` stages what it renders. A
/// template git does not carry would put a served file on the tag with no
/// source beside it. `check` would also have nothing to compare against on a
/// fresh clone.
///
/// A template that is present and untracked stops the run instead of rendering
/// nothing. Without that stop, adding a served file would give silence and
/// serve the stale file.
fn sources(root: &Path, config: &Templates) -> Result<Vec<PathBuf>> {
    let tracked: Vec<PathBuf> = git::tracked(root)?
        .into_iter()
        .filter(|path| is_template(path, config))
        .collect();
    if git::is_repository(root) {
        refuse_untracked(root, config, &tracked)?;
    }
    Ok(tracked)
}

fn is_template(path: &Path, config: &Templates) -> bool {
    path.starts_with(&config.source)
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(&config.suffix))
}

/// A template present in the source tree that the repo neither tracks nor
/// ignores was written and forgotten. One it ignores was written and disowned,
/// which is a repo saying it is not part of what it publishes. That is the same
/// thing `git::tracked` reads the index for, so that one is left alone.
fn refuse_untracked(root: &Path, config: &Templates, tracked: &[PathBuf]) -> Result<()> {
    let loose: Vec<String> = present(&root.join(&config.source), &config.suffix)?
        .iter()
        .filter_map(|path| path.strip_prefix(root).ok())
        .filter(|path| !tracked.iter().any(|known| known == path))
        .filter(|path| !git::is_ignored(root, path))
        .map(|path| path.display().to_string())
        .collect();
    if loose.is_empty() {
        return Ok(());
    }
    bail!(
        "a template git does not track renders nothing: {} — commit it or delete it",
        loose.join(", ")
    );
}

/// Every template in `dir` and its subdirectories. A directory that is not
/// there holds none: a repo serving nothing writes no source tree.
///
/// Symlinks are skipped instead of followed. One pointing at an ancestor could
/// otherwise recurse forever, and one pointing outside could pull a foreign
/// file into this repo's answer. `ensure_inside` refuses a symlinked target for
/// the same reason.
fn present(dir: &Path, suffix: &str) -> Result<Vec<PathBuf>> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(Vec::new());
    };
    let mut found = Vec::new();
    for entry in entries {
        let path = entry.with_context(|| dir.display().to_string())?.path();
        let kind = fs::symlink_metadata(&path)
            .with_context(|| path.display().to_string())?
            .file_type();
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            found.extend(present(&path, suffix)?);
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(suffix))
        {
            found.push(path);
        }
    }
    found.sort();
    Ok(found)
}

/// A target a template may write: inside the repository, not through a
/// symlink, and not escaping upward.
fn ensure_inside(root: &Path, target: &Path) -> Result<()> {
    if target.is_absolute()
        || target
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        bail!("target must stay inside the repo: {}", target.display());
    }
    let path = root.join(target);
    if let Ok(meta) = fs::symlink_metadata(&path)
        && meta.file_type().is_symlink()
    {
        bail!("target must not be a symlink: {}", path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use tempfile::TempDir;

    use super::*;
    use crate::test_git;

    /// The one template every version case below renders.
    const TEMPLATE: &str = indoc! {r#"
        {%- set path = "tasks/ver" -%}
        #!/usr/bin/env bash
        #MISE tools = { "github:victor-software-house/verctl" = "{{ versions["verctl"] }}", "cargo:ctl-core" = "{{ versions["ctl-core"] }}" }

        exec verctl "$@"
    "#};

    /// The one served file every version case below starts from.
    const BEFORE: &str = indoc! {r#"
        #!/usr/bin/env bash
        #MISE tools = { "github:victor-software-house/verctl" = "0.0.1", "cargo:ctl-core" = "1.0.0" }

        exec verctl "$@"
    "#};

    const SERVED: &str = "tasks/ver/ver";
    const TEMPLATE_PATH: &str = ".ctl/templates/ver.jinja";

    /// What a release does to `BEFORE`: the exact bytes, or the failure.
    enum Outcome {
        Serves(&'static str),
        Fails(&'static str),
    }

    fn versions(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(name, version)| ((*name).to_owned(), (*version).to_owned()))
            .collect()
    }

    /// A repo whose index holds exactly `files`, each with the given body.
    fn repo(files: &[(&str, &str)]) -> TempDir {
        let root = TempDir::new().unwrap();
        test_git::init(root.path());
        for (path, body) in files {
            let full = root.path().join(path);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(&full, body).unwrap();
            test_git::git(root.path(), &["add", "-f", "--", path]);
        }
        root
    }

    fn read(root: &TempDir, path: &str) -> String {
        fs::read_to_string(root.path().join(path)).unwrap()
    }

    /// One template, one before, many releases: each asserted whole.
    #[test]
    fn one_template_serves_every_release_exactly() {
        let cases = [
            (
                "both packages move",
                &[("verctl", "0.0.2"), ("ctl-core", "1.2.3")][..],
                Outcome::Serves(indoc! {r#"
                    #!/usr/bin/env bash
                    #MISE tools = { "github:victor-software-house/verctl" = "0.0.2", "cargo:ctl-core" = "1.2.3" }

                    exec verctl "$@"
                "#}),
            ),
            (
                "only the tool moves",
                &[("verctl", "1.0.0"), ("ctl-core", "1.0.0")][..],
                Outcome::Serves(indoc! {r#"
                    #!/usr/bin/env bash
                    #MISE tools = { "github:victor-software-house/verctl" = "1.0.0", "cargo:ctl-core" = "1.0.0" }

                    exec verctl "$@"
                "#}),
            ),
            (
                "a release that changes nothing still serves the same bytes",
                &[("verctl", "0.0.1"), ("ctl-core", "1.0.0")][..],
                Outcome::Serves(BEFORE),
            ),
            (
                "a package the template names is not in the release",
                &[("verctl", "0.0.2")][..],
                Outcome::Fails(TEMPLATE_PATH),
            ),
            ("no packages at all", &[][..], Outcome::Fails(TEMPLATE_PATH)),
        ];
        for (label, pairs, outcome) in cases {
            let root = repo(&[(TEMPLATE_PATH, TEMPLATE), (SERVED, BEFORE)]);
            let result = write(root.path(), &Templates::default(), &[], &versions(pairs));
            match outcome {
                Outcome::Serves(after) => {
                    assert_eq!(result.unwrap(), [root.path().join(SERVED)], "{label}");
                    assert_eq!(read(&root, SERVED), after, "{label}");
                }
                Outcome::Fails(named) => {
                    let report = format!("{:#}", result.unwrap_err());
                    assert!(report.contains(named), "{label}: {report}");
                    assert_eq!(read(&root, SERVED), BEFORE, "{label}");
                }
            }
            assert_eq!(read(&root, TEMPLATE_PATH), TEMPLATE, "{label}");
        }
    }

    /// The Version PR is regenerated on every push to main, so a second render
    /// of the same versions must produce the same bytes.
    #[test]
    fn rendering_twice_serves_the_same_bytes() {
        let root = repo(&[(TEMPLATE_PATH, TEMPLATE), (SERVED, BEFORE)]);
        let moved = versions(&[("verctl", "0.0.2"), ("ctl-core", "1.2.3")]);
        write(root.path(), &Templates::default(), &[], &moved).unwrap();
        let once = read(&root, SERVED);
        write(root.path(), &Templates::default(), &[], &moved).unwrap();
        assert_eq!(read(&root, SERVED), once);
    }

    #[test]
    fn a_plan_reports_the_served_file_without_writing_it() {
        let root = repo(&[(TEMPLATE_PATH, TEMPLATE), (SERVED, BEFORE)]);
        assert_eq!(
            plan(
                root.path(),
                &Templates::default(),
                &[],
                &versions(&[("verctl", "0.0.2"), ("ctl-core", "1.2.3")])
            )
            .unwrap(),
            [root.path().join(SERVED)]
        );
        assert_eq!(read(&root, SERVED), BEFORE);
    }

    #[test]
    fn a_broken_template_fails_before_writing_the_served_file() {
        let root = repo(&[(TEMPLATE_PATH, "pin {{ versions[\n"), (SERVED, BEFORE)]);
        assert!(
            write(
                root.path(),
                &Templates::default(),
                &[],
                &versions(&[("verctl", "0.0.2")])
            )
            .is_err()
        );
        assert_eq!(read(&root, SERVED), BEFORE);
    }

    /// `check` compares each served file with its rendering and names every
    /// one that differs, a missing file included.
    #[test]
    fn drift_names_each_served_file_that_differs() {
        let current = versions(&[("verctl", "0.0.1"), ("ctl-core", "1.0.0")]);
        let root = repo(&[(TEMPLATE_PATH, TEMPLATE), (SERVED, BEFORE)]);
        assert_eq!(
            drift(root.path(), &Templates::default(), &[], &current).unwrap(),
            Vec::<Drift>::new()
        );
        fs::write(root.path().join(SERVED), BEFORE.replace("0.0.1", "0.0.9")).unwrap();
        let expected = [Drift {
            target: PathBuf::from(SERVED),
            template: PathBuf::from(TEMPLATE_PATH),
        }];
        assert_eq!(
            drift(root.path(), &Templates::default(), &[], &current).unwrap(),
            expected
        );
        fs::remove_file(root.path().join(SERVED)).unwrap();
        assert_eq!(
            drift(root.path(), &Templates::default(), &[], &current).unwrap(),
            expected
        );
    }

    /// A template and a `pins` entry that claim one file stop the run before
    /// anything is written.
    /// The same file spelled with a leading `./` on either side still has two
    /// owners.
    #[test]
    fn a_target_a_pin_also_rewrites_is_refused() {
        let dotted = TEMPLATE.replace(r#""tasks/ver""#, r#""./tasks/ver""#);
        for (pin_file, template) in [
            (SERVED, TEMPLATE),
            ("./tasks/ver/ver", TEMPLATE),
            (SERVED, dotted.as_str()),
        ] {
            let root = repo(&[(TEMPLATE_PATH, template), (SERVED, BEFORE)]);
            let pin = Pin {
                file: PathBuf::from(pin_file),
                tool: None,
                pattern_ids: Vec::new(),
                patterns: Vec::new(),
                package: "verctl".to_owned(),
            };
            let error = write(
                root.path(),
                &Templates::default(),
                &[pin],
                &versions(&[("verctl", "0.0.2"), ("ctl-core", "1.2.3")]),
            )
            .unwrap_err();
            let message = format!("{error:#}");
            assert!(
                message.contains("tasks/ver/ver has two owners"),
                "{pin_file}: {message}"
            );
            assert!(message.contains(TEMPLATE_PATH), "{pin_file}: {message}");
            assert!(message.contains("package verctl"), "{pin_file}: {message}");
            assert_eq!(read(&root, SERVED), BEFORE, "{pin_file}");
        }
    }

    /// Which pairs exist at all. Here the tree varies and the versions stay
    /// fixed.
    #[test]
    fn an_untracked_template_stops_the_run_and_one_outside_is_ignored() {
        let moved = versions(&[("verctl", "0.0.2"), ("ctl-core", "1.2.3")]);

        let stray_template = repo(&[(SERVED, BEFORE)]);
        let path = stray_template.path().join(TEMPLATE_PATH);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, TEMPLATE).unwrap();
        let error = write(stray_template.path(), &Templates::default(), &[], &moved).unwrap_err();
        let text = format!("{error:#}");
        assert!(text.contains(TEMPLATE_PATH), "{text}");
        assert!(text.contains("commit it or delete it"), "{text}");
        assert_eq!(
            read(&stray_template, SERVED),
            BEFORE,
            "the served file is left alone: the run stops before anything is written"
        );

        let outside = repo(&[("templates/changelog.jinja", "# {{ anything }}\n")]);
        assert_eq!(
            write(outside.path(), &Templates::default(), &[], &moved).unwrap(),
            Vec::<PathBuf>::new()
        );
        assert_eq!(
            read(&outside, "templates/changelog.jinja"),
            "# {{ anything }}\n"
        );
    }

    /// The layout does not matter, so the search ignores it. A template nested
    /// under the source tree counts. Every loose one is named together instead
    /// of one per release.
    #[test]
    fn every_untracked_template_is_named_at_whatever_depth_it_sits() {
        let root = repo(&[(SERVED, BEFORE)]);
        for path in [
            ".ctl/templates/ver.jinja",
            ".ctl/templates/nested/deeper/readme.jinja",
        ] {
            let full = root.path().join(path);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(&full, TEMPLATE).unwrap();
        }
        fs::write(root.path().join(".ctl/templates/notes.md"), "not one\n").unwrap();
        let error = write(
            root.path(),
            &Templates::default(),
            &[],
            &versions(&[("verctl", "0.0.2"), ("ctl-core", "1.2.3")]),
        )
        .unwrap_err();
        let text = format!("{error:#}");
        assert!(text.contains(".ctl/templates/ver.jinja"), "{text}");
        assert!(
            text.contains(".ctl/templates/nested/deeper/readme.jinja"),
            "{text}"
        );
        assert!(
            !text.contains("notes.md"),
            "a file without the suffix is not a template: {text}"
        );
    }

    /// One template body, every declaration a repo might write: what the
    /// schema accepts, where it lands, and what it refuses.
    #[test]
    fn the_schema_decides_where_a_template_lands() {
        let cases = [
            (
                "nothing declared: the template's own name, at the root",
                "",
                Ok("ver"),
            ),
            (
                "a directory",
                r#"{%- set path = "tasks/ver" -%}"#,
                Ok("tasks/ver/ver"),
            ),
            (
                "a name",
                r#"{%- set name = "mise.toml" -%}"#,
                Ok("mise.toml"),
            ),
            (
                "both",
                indoc! {r#"
                    {%- set path = "examples" -%}
                    {%- set name = "mise.toml" -%}
                "#},
                Ok("examples/mise.toml"),
            ),
            (
                "a computed declaration is still a declaration",
                r#"{%- set path = "tasks/" ~ "ver" -%}"#,
                Ok("tasks/ver/ver"),
            ),
            (
                "a path that climbs out",
                r#"{%- set path = "../escaped" -%}"#,
                Err("path: must stay inside the repository"),
            ),
            (
                "an absolute path",
                r#"{%- set path = "/etc" -%}"#,
                Err("path: must stay inside the repository"),
            ),
            (
                "a name that is a path",
                r#"{%- set name = "tasks/ver" -%}"#,
                Err("name: must be one file name"),
            ),
            (
                "a name that climbs out",
                r#"{%- set name = ".." -%}"#,
                Err("name: must be one file name"),
            ),
            (
                "a path of the wrong type",
                r"{%- set path = 5 -%}",
                Err("invalid type"),
            ),
            (
                "an executable of the wrong type",
                r#"{%- set executable = "yes" -%}"#,
                Err("expected a boolean"),
            ),
        ];
        for (label, declaration, expected) in cases {
            let root = repo(&[(TEMPLATE_PATH, &format!("{declaration}served\n"))]);
            let result = write(
                root.path(),
                &Templates::default(),
                &[],
                &versions(&[("verctl", "0.0.2")]),
            );
            match expected {
                Ok(target) => {
                    assert_eq!(result.unwrap(), [root.path().join(target)], "{label}");
                    assert_eq!(read(&root, target), "served\n", "{label}");
                }
                Err(complaint) => {
                    let report = format!("{:#}", result.unwrap_err());
                    assert!(report.contains(complaint), "{label}: {report}");
                    assert!(report.contains(TEMPLATE_PATH), "{label}: {report}");
                }
            }
        }
    }

    /// A template's other variables are its own business. A `tool` it uses to
    /// build a line is not a declaration and must not fail the schema.
    #[test]
    fn an_export_the_schema_does_not_know_is_the_templates_own() {
        let root = repo(&[(
            TEMPLATE_PATH,
            indoc! {r#"
                {%- set tool = "github:victor-software-house/verctl" -%}
                {%- set path = "tasks/ver" -%}
                {{ tool }} = "{{ versions["verctl"] }}"
            "#},
        )]);
        write(
            root.path(),
            &Templates::default(),
            &[],
            &versions(&[("verctl", "0.0.2")]),
        )
        .unwrap();
        assert_eq!(
            read(&root, "tasks/ver/ver"),
            "github:victor-software-house/verctl = \"0.0.2\"\n"
        );
    }

    /// The target's own path, however deep, with no template beside it: a
    /// README template does not sit in the repo root next to `README.md`.
    #[test]
    fn a_flat_template_names_a_target_at_any_depth() {
        let root = repo(&[
            (
                ".ctl/templates/readme.jinja",
                "{%- set name = \"README.md\" -%}\nRun verctl@{{ versions[\"verctl\"] }}.\n",
            ),
            ("README.md", "Run verctl@0.0.1.\n"),
            (TEMPLATE_PATH, TEMPLATE),
            (SERVED, BEFORE),
        ]);
        let mut rendered = write(
            root.path(),
            &Templates::default(),
            &[],
            &versions(&[("verctl", "0.0.2"), ("ctl-core", "1.2.3")]),
        )
        .unwrap();
        rendered.sort();
        assert_eq!(
            rendered,
            [root.path().join("README.md"), root.path().join(SERVED)]
        );
        assert_eq!(read(&root, "README.md"), "Run verctl@0.0.2.\n");
        assert!(!root.path().join("README.md.jinja").exists());
        assert!(!root.path().join(".ctl/templates/tasks").exists());
    }

    /// The one mode bit git records, declared by the template that needs it.
    /// A served task file is executable; the README beside it is not.
    #[test]
    fn a_template_declares_the_one_mode_bit_git_records() {
        let root = repo(&[
            (
                ".ctl/templates/ver.jinja",
                indoc! {r#"
                    {%- set path = "tasks/ver" -%}
                    {%- set executable = true -%}
                    #!/usr/bin/env bash
                    exec verctl@{{ versions["verctl"] }} "$@"
                "#},
            ),
            (SERVED, BEFORE),
            (
                ".ctl/templates/readme.jinja",
                indoc! {r#"
                    {%- set name = "README.md" -%}
                    {%- set executable = false -%}
                    Run verctl@{{ versions["verctl"] }}.
                "#},
            ),
            ("README.md", "Run verctl@0.0.1.\n"),
        ]);
        write(
            root.path(),
            &Templates::default(),
            &[],
            &versions(&[("verctl", "0.0.2")]),
        )
        .unwrap();
        let mode = |path: &str| {
            fs::metadata(root.path().join(path))
                .unwrap()
                .permissions()
                .mode()
                & 0o777
        };
        assert_eq!(mode(SERVED), 0o755);
        assert_eq!(mode("README.md"), 0o644);
        assert!(
            read(&root, SERVED).starts_with("#!/usr/bin/env bash\n"),
            "{}",
            read(&root, SERVED)
        );
    }

    /// An undeclared mode does not mean "leave it". A served file's mode is
    /// part of what the tag carries. The default is 0644, so a stale 0755 is
    /// corrected.
    #[test]
    fn a_template_that_says_nothing_about_mode_serves_0644() {
        let root = repo(&[(TEMPLATE_PATH, TEMPLATE), (SERVED, BEFORE)]);
        fs::set_permissions(root.path().join(SERVED), fs::Permissions::from_mode(0o755)).unwrap();
        write(
            root.path(),
            &Templates::default(),
            &[],
            &versions(&[("verctl", "0.0.2"), ("ctl-core", "1.2.3")]),
        )
        .unwrap();
        assert_eq!(
            fs::metadata(root.path().join(SERVED))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o644
        );
    }

    #[test]
    fn the_source_tree_and_suffix_are_configurable() {
        let root = repo(&[("served/tasks/ver/ver.tmpl", TEMPLATE), (SERVED, BEFORE)]);
        let config = Templates {
            source: PathBuf::from("served"),
            suffix: ".tmpl".to_owned(),
        };
        let rendered = write(
            root.path(),
            &config,
            &[],
            &versions(&[("verctl", "0.0.2"), ("ctl-core", "1.2.3")]),
        )
        .unwrap();
        assert_eq!(rendered, [root.path().join(SERVED)]);
        assert_eq!(
            read(&root, SERVED),
            indoc! {r#"
                #!/usr/bin/env bash
                #MISE tools = { "github:victor-software-house/verctl" = "0.0.2", "cargo:ctl-core" = "1.2.3" }

                exec verctl "$@"
            "#}
        );
    }

    /// One repository may hold several projects. The index lists paths from
    /// the working tree. A project in a subdirectory has to be found by the
    /// path it uses itself, and a sibling project's templates are not its.
    #[test]
    fn a_project_below_the_repository_root_serves_its_own_templates() {
        let root = repo(&[
            (&format!("tool/{TEMPLATE_PATH}"), TEMPLATE),
            (&format!("tool/{SERVED}"), BEFORE),
            (&format!("other/{TEMPLATE_PATH}"), TEMPLATE),
        ]);
        let project = root.path().join("tool");
        assert_eq!(
            write(
                &project,
                &Templates::default(),
                &[],
                &versions(&[("verctl", "0.0.2"), ("ctl-core", "0.1.0")])
            )
            .unwrap(),
            [project.join(SERVED)],
            "the project's own template, named the way the project names it"
        );
        assert_eq!(
            fs::read_to_string(root.path().join(format!("other/{SERVED}"))).ok(),
            None,
            "a sibling project's template is not this one's to render"
        );
    }

    /// A repo that serves nothing is detected before anything is read on its
    /// behalf. Rendering needs every package's version, and a repo with no
    /// template must not pay for that map.
    #[test]
    fn a_repo_with_no_template_serves_nothing() {
        let bare = repo(&[("README.md", "nothing to serve\n")]);
        assert!(!any(bare.path(), &Templates::default()).unwrap());
        let serving = repo(&[(TEMPLATE_PATH, TEMPLATE)]);
        assert!(any(serving.path(), &Templates::default()).unwrap());
    }

    /// Ignoring a file is a repo disowning it. That is the same statement
    /// `tracked` reads the index for, so the author is never required to commit
    /// the file.
    #[test]
    fn an_ignored_template_is_disowned_rather_than_forgotten() {
        let root = repo(&[(SERVED, BEFORE), (".gitignore", "*.local.jinja\n")]);
        let scratch = root.path().join(".ctl/templates/draft.local.jinja");
        fs::create_dir_all(scratch.parent().unwrap()).unwrap();
        fs::write(&scratch, TEMPLATE).unwrap();
        assert_eq!(
            write(
                root.path(),
                &Templates::default(),
                &[],
                &versions(&[("verctl", "0.0.2"), ("ctl-core", "1.2.3")])
            )
            .unwrap(),
            Vec::<PathBuf>::new()
        );
        assert_eq!(read(&root, SERVED), BEFORE);
    }

    /// A symlink is not followed. A directory outside this repository therefore
    /// cannot have its templates counted as ours. That would require whoever
    /// ran the release to commit a file that is not theirs.
    #[test]
    fn a_symlink_out_of_the_repository_serves_nothing_of_its_own() {
        let elsewhere = TempDir::new().unwrap();
        fs::write(elsewhere.path().join("foreign.jinja"), TEMPLATE).unwrap();

        let root = repo(&[(SERVED, BEFORE)]);
        let templates = root.path().join(".ctl/templates");
        fs::create_dir_all(&templates).unwrap();
        std::os::unix::fs::symlink(elsewhere.path(), templates.join("linked")).unwrap();
        assert_eq!(
            write(
                root.path(),
                &Templates::default(),
                &[],
                &versions(&[("verctl", "0.0.2"), ("ctl-core", "1.2.3")])
            )
            .unwrap(),
            Vec::<PathBuf>::new()
        );
    }

    /// A template here is not untracked: there is no index to not carry it.
    /// Such a tree cannot serve a file by tag at all. It renders nothing and
    /// reports nothing, instead of failing the way a repository would, and
    /// `version` still works there. A repository whose index cannot be read
    /// is a different case, and `git::tracked` reports that as an error.
    #[test]
    fn a_tree_with_no_repository_has_no_templates() {
        let root = TempDir::new().unwrap();
        let path = root.path().join(TEMPLATE_PATH);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, TEMPLATE).unwrap();
        assert_eq!(
            write(
                root.path(),
                &Templates::default(),
                &[],
                &versions(&[("verctl", "0.0.2")])
            )
            .unwrap(),
            Vec::<PathBuf>::new()
        );
    }
}
