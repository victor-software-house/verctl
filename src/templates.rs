//! Render the files a repo serves, from templates that say what they are for.
//!
//! Every template lives flat in `.verctl/templates/` and **declares where it
//! goes**, in Jinja's own syntax rather than a manifest beside it or a mirrored
//! directory tree:
//!
//! ```jinja
//! {%- set path = "tasks/q" -%}
//! {%- set executable = true -%}
//! #!/usr/bin/env bash
//! #MISE tools = { "github:victor-software-house/qctl" = "{{ versions["qctl"] }}" }
//! ```
//!
//! Jinja has no frontmatter, but a top-level `{% set %}` is its equivalent: an
//! export, readable from the evaluated state, so the metadata travels inside
//! the file it describes. `q.jinja` above serves `tasks/q/q` — the file name
//! defaults to the template's own, so most templates declare one line, and
//! nothing accumulates in configuration.
//!
//! `Declared` is the whole schema, parsed once and resolved into a checked
//! `Served`. Trim declarations with `{%- … -%}`: a shebang has to stay on line
//! one.

use crate::config::Templates;
use crate::git;
use crate::schema::{inside_the_repo, one_file_name};
use anyhow::{Context, Result, bail};
use garde::Validate;
use minijinja::value::Value;
use minijinja::{Environment, State, UndefinedBehavior, context};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};

/// What a template declares about the file it serves, in Jinja's own syntax:
/// top-level `{%- set … -%}` exports, read as one schema and checked at the
/// boundary. Other exports are the template's own business — a name it uses to
/// build a line is not a declaration.
#[derive(Debug, Deserialize, Validate)]
#[garde(allow_unvalidated)]
struct Declared {
    /// The directory the target goes in. Unsaid: the repository root.
    #[serde(default)]
    #[garde(custom(inside_the_repo))]
    path: PathBuf,
    /// The target file's name. Unsaid, it is this template's own name without
    /// the suffix, which is what a served file almost always wants — so the
    /// fallback is seeded before parsing and this is never absent.
    #[garde(custom(one_file_name))]
    name: String,
    /// Whether the target is executable. Git records one bit of mode for a
    /// regular file and nothing else — "only 0755 and 0644 are valid for
    /// regular files" (gitformat-index) — and its other modes are not files a
    /// template renders: 120000 is a symlink, 160000 a gitlink, 040000 a tree.
    /// So there is no third state, and no mode field: unsaid means 0644.
    #[serde(default)]
    executable: bool,
}

pub fn write(
    root: &Path,
    config: &Templates,
    versions: &[(String, String)],
) -> Result<Vec<PathBuf>> {
    render(root, config, versions, true)
}

/// Same checks as `write`, without touching disk.
pub fn plan(
    root: &Path,
    config: &Templates,
    versions: &[(String, String)],
) -> Result<Vec<PathBuf>> {
    render(root, config, versions, false)
}

fn render(
    root: &Path,
    config: &Templates,
    versions: &[(String, String)],
    persist: bool,
) -> Result<Vec<PathBuf>> {
    let context = context! {
        versions => versions
            .iter()
            .map(|(name, version)| (name.clone(), version.clone()))
            .collect::<BTreeMap<_, _>>(),
    };
    let mut environment = Environment::new();
    environment.set_undefined_behavior(UndefinedBehavior::Strict);
    environment.set_keep_trailing_newline(true);

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
        let body = captured.output();
        let declared = Declared::parse(&template, config, captured.state())?;
        let target = declared.target();
        ensure_inside(root, &target).with_context(|| format!("template {}", template.display()))?;
        let path = root.join(&target);
        if persist {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("create {}", parent.display()))?;
            }
            fs::write(&path, body).with_context(|| format!("write {}", target.display()))?;
            let mode = if declared.executable { 0o755 } else { 0o644 };
            fs::set_permissions(&path, fs::Permissions::from_mode(mode))
                .with_context(|| format!("chmod {}", target.display()))?;
        }
        rendered.push(path);
    }
    Ok(rendered)
}

impl Declared {
    /// Read the whole schema out of an evaluated template at once, over a
    /// seeded `name`. Types and shapes are the schema's business: `path = 5`
    /// and `name = "a/b"` fail here, not at write time.
    fn parse(template: &Path, config: &Templates, state: &State) -> Result<Self> {
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

/// Every tracked template in the source tree. Flat or nested, the layout says
/// nothing: each template declares its own target.
fn sources(root: &Path, config: &Templates) -> Result<Vec<PathBuf>> {
    Ok(git::tracked(root)?
        .into_iter()
        .filter(|path| {
            path.starts_with(&config.source)
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with(&config.suffix))
        })
        .collect())
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use git2::Repository;
    use indoc::indoc;
    use tempfile::TempDir;

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
    const TEMPLATE_PATH: &str = ".verctl/templates/ver.jinja";

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
        let repository = Repository::init(root.path()).unwrap();
        let mut index = repository.index().unwrap();
        for (path, body) in files {
            let full = root.path().join(path);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(&full, body).unwrap();
            index.add_path(Path::new(path)).unwrap();
        }
        index.write().unwrap();
        root
    }

    fn read(root: &TempDir, path: &str) -> String {
        fs::read_to_string(root.path().join(path)).unwrap()
    }

    /// One template, one before, many releases — each asserted whole.
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
            let result = write(root.path(), &Templates::default(), &versions(pairs));
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
        write(root.path(), &Templates::default(), &moved).unwrap();
        let once = read(&root, SERVED);
        write(root.path(), &Templates::default(), &moved).unwrap();
        assert_eq!(read(&root, SERVED), once);
    }

    #[test]
    fn a_plan_reports_the_served_file_without_writing_it() {
        let root = repo(&[(TEMPLATE_PATH, TEMPLATE), (SERVED, BEFORE)]);
        assert_eq!(
            plan(
                root.path(),
                &Templates::default(),
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
                &versions(&[("verctl", "0.0.2")])
            )
            .is_err()
        );
        assert_eq!(read(&root, SERVED), BEFORE);
    }

    /// Which pairs exist at all: the tree varies here, not the versions.
    #[test]
    fn only_a_tracked_template_in_the_source_tree_is_rendered() {
        let moved = versions(&[("verctl", "0.0.2"), ("ctl-core", "1.2.3")]);

        let stray_template = repo(&[(SERVED, BEFORE)]);
        let path = stray_template.path().join(TEMPLATE_PATH);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, TEMPLATE).unwrap();
        assert_eq!(
            write(stray_template.path(), &Templates::default(), &moved).unwrap(),
            Vec::<PathBuf>::new()
        );
        assert_eq!(read(&stray_template, SERVED), BEFORE);

        let outside = repo(&[("templates/changelog.jinja", "# {{ anything }}\n")]);
        assert_eq!(
            write(outside.path(), &Templates::default(), &moved).unwrap(),
            Vec::<PathBuf>::new()
        );
        assert_eq!(
            read(&outside, "templates/changelog.jinja"),
            "# {{ anything }}\n"
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

    /// A template's other variables are its own business: a `tool` it uses to
    /// build a line is not a declaration, and must not fail the schema.
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
                ".verctl/templates/readme.jinja",
                "{%- set name = \"README.md\" -%}\nRun verctl@{{ versions[\"verctl\"] }}.\n",
            ),
            ("README.md", "Run verctl@0.0.1.\n"),
            (TEMPLATE_PATH, TEMPLATE),
            (SERVED, BEFORE),
        ]);
        let mut rendered = write(
            root.path(),
            &Templates::default(),
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
        assert!(!root.path().join(".verctl/templates/tasks").exists());
    }

    /// The one mode bit git records, declared by the template that needs it.
    /// A served task file is executable; the README beside it is not.
    #[test]
    fn a_template_declares_the_one_mode_bit_git_records() {
        let root = repo(&[
            (
                ".verctl/templates/ver.jinja",
                indoc! {r#"
                    {%- set path = "tasks/ver" -%}
                    {%- set executable = true -%}
                    #!/usr/bin/env bash
                    exec verctl@{{ versions["verctl"] }} "$@"
                "#},
            ),
            (SERVED, BEFORE),
            (
                ".verctl/templates/readme.jinja",
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

    /// Unsaid is not "leave it": a served file's mode is part of what the tag
    /// carries, so silence means 0644 and a stale 0755 is corrected.
    #[test]
    fn a_template_that_says_nothing_about_mode_serves_0644() {
        let root = repo(&[(TEMPLATE_PATH, TEMPLATE), (SERVED, BEFORE)]);
        fs::set_permissions(root.path().join(SERVED), fs::Permissions::from_mode(0o755)).unwrap();
        write(
            root.path(),
            &Templates::default(),
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

    /// A tree with no repository cannot serve a file by tag, so it has no
    /// templates — `prepare` still works there, as it did before templates
    /// existed. A repository whose index cannot be read is a different case,
    /// and `git::tracked` reports that as an error.
    /// One repository may hold several projects. The index speaks in paths from
    /// the working tree, so a project in a subdirectory has to be found by the
    /// path it uses itself — and a sibling project's templates are not its.
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

    #[test]
    fn a_tree_with_no_repository_has_no_templates() {
        let root = TempDir::new().unwrap();
        assert_eq!(
            write(
                root.path(),
                &Templates::default(),
                &versions(&[("verctl", "0.0.2")])
            )
            .unwrap(),
            Vec::<PathBuf>::new()
        );
    }
}
