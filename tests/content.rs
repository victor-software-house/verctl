//! The check on the content of a Version PR. A branch passes when every change
//! is one the version step makes from the merge-base. Every other change fails
//! and is named.
//!
//! Each fixture commits a merge-base on `main`. It then builds the Version PR
//! on `verctl/release/main` with the version step's own library calls. The
//! expected bytes therefore come from the code under test.

#![expect(
    clippy::expect_used,
    reason = "the fixture helpers fail fast, as in tests/common.rs"
)]

mod common;

use std::fs;
use std::path::Path;

use indoc::{formatdoc, indoc};
use tempfile::TempDir;
use verctl::config::{self, Config};
use verctl::{content, fragment, git, pins, prepare, pull_request, release, templates, versions};

const ORIGIN: &str = "https://github.com/victor-software-house/demo.git";

/// A repository whose merge-base is on `main`, tracked as `origin/main`.
struct Repo(TempDir);

impl Repo {
    fn new(config: &str) -> Self {
        let dir = TempDir::new().expect("tempdir");
        common::init(dir.path());
        common::write_config(dir.path(), config);
        Self(dir)
    }

    fn root(&self) -> &Path {
        self.0.path()
    }

    fn write(&self, relative: &str, body: &str) {
        let path = self.root().join(relative);
        fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        fs::write(path, body).expect("write");
    }

    fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.root().join(relative)).expect("read")
    }

    fn copy(&self, name: &str, relative: &str) {
        self.write(relative, &testdata(name));
    }

    /// Commit the merge-base on `main`, track it as `origin/main`, and start
    /// the Version PR branch from it.
    fn branch(&self) {
        let sha = common::commit_all(self.root(), "base");
        common::git(self.root(), &["remote", "add", "origin", ORIGIN]);
        common::git(
            self.root(),
            &["update-ref", "refs/remotes/origin/main", &sha],
        );
        common::git(
            self.root(),
            &["checkout", "-q", "-b", "verctl/release/main"],
        );
    }

    /// What `verctl version` writes before it runs any command: manifests,
    /// pins, served files, CHANGELOG sections, and consumed fragments.
    fn version_step(&self) {
        let root = self.root();
        let config = self.config();
        let fragments = fragment::load_dir(&root.join(".changeset")).expect("fragments");
        let plan = prepare::plan(&config, &fragments, root).expect("plan");
        let planned: Vec<(String, String)> = plan
            .iter()
            .map(|entry| (entry.name.clone(), entry.to.clone()))
            .collect();
        prepare::apply_plan(&plan).expect("apply");
        pins::write(root, &config.pins, &planned).expect("pins");
        if templates::any(root, &config.templates).expect("templates") {
            let versions = release::served_versions(root, &config, &planned);
            templates::write(root, &config.templates, &config.pins, &versions).expect("render");
        }
        release::write_changelogs(&config, root, &plan, &fragments).expect("changelogs");
        release::consume_fragments(release::contributing_fragments(&plan, &fragments))
            .expect("consume");
    }

    fn commit(&self) {
        common::commit_all(self.root(), "version");
    }

    fn config(&self) -> Config {
        Config::load(&self.root().join(config::FILE)).expect("config")
    }

    fn check(&self) -> anyhow::Result<Vec<String>> {
        let inputs = content::Inputs::new(
            self.root().join(config::FILE),
            self.root().join(".changeset"),
        );
        content::check(
            self.root(),
            &self.config(),
            &inputs,
            &versions::stock_candidates(),
        )
    }

    fn passes(&self) {
        if let Err(error) = self.check() {
            panic!("{error:#}");
        }
    }

    /// The check fails, and its message names each of `names`.
    fn fails_naming(&self, names: &[&str]) {
        let message = format!("{:#}", self.check().expect_err("the check passed"));
        for name in names {
            assert!(message.contains(name), "{name} missing from: {message}");
        }
    }
}

/// A fragment holding `bumps`, one `package: bump` line.
fn fragment(bumps: &str) -> String {
    formatdoc! {"
        ---
        {bumps}
        ---

        Change.
    "}
}

const DEMO_CONFIG: &str = indoc! {"
    packages:
      - name: demo
        path: Cargo.toml
"};

/// `demo` 1.0.0 at `Cargo.toml` with a CHANGELOG, and `pending` as its only
/// fragment when there is one.
fn crate_repo(pending: Option<&str>) -> Repo {
    let repo = Repo::new(DEMO_CONFIG);
    repo.write(
        "Cargo.toml",
        indoc! {r#"
            [package]
            name = "demo"
            version = "1.0.0"

            [dependencies]
            foo = "1.0"
        "#},
    );
    repo.write("src/lib.rs", "");
    repo.write(
        "CHANGELOG.md",
        indoc! {"
            # Changelog

            ## 1.0.0

            - Start.
        "},
    );
    if let Some(bumps) = pending {
        repo.write(".changeset/fix.md", &fragment(bumps));
    }
    repo.branch();
    repo
}

#[test]
fn a_version_pr_that_applies_the_plan_passes() {
    let repo = crate_repo(Some("demo: patch"));
    repo.version_step();
    repo.commit();
    assert!(repo.read("Cargo.toml").contains(r#"version = "1.0.1""#));
    repo.passes();
}

#[test]
fn a_version_pr_that_carries_a_source_change_fails() {
    let repo = crate_repo(Some("demo: patch"));
    repo.version_step();
    repo.write("src/main.rs", "fn main() {}");
    repo.commit();
    repo.fails_naming(&["src/main.rs", "does not write"]);
}

/// A symlink holds no bytes the version step writes, so retargeting one is a
/// change the version step does not make.
#[test]
fn a_version_pr_that_retargets_a_symlink_fails() {
    let repo = crate_repo(Some("demo: patch"));
    std::os::unix::fs::symlink("Cargo.toml", repo.root().join("link")).expect("symlink");
    common::commit_all(repo.root(), "link");
    common::git(
        repo.root(),
        &["update-ref", "refs/remotes/origin/main", "HEAD"],
    );
    repo.version_step();
    fs::remove_file(repo.root().join("link")).expect("remove");
    std::os::unix::fs::symlink("CHANGELOG.md", repo.root().join("link")).expect("symlink");
    repo.commit();
    repo.fails_naming(&["link", "does not write"]);
}

/// An uncommitted retarget counts too: the local version branch checks the
/// working tree.
#[test]
fn an_uncommitted_symlink_retarget_fails() {
    let repo = crate_repo(Some("demo: patch"));
    std::os::unix::fs::symlink("Cargo.toml", repo.root().join("link")).expect("symlink");
    common::commit_all(repo.root(), "link");
    common::git(
        repo.root(),
        &["update-ref", "refs/remotes/origin/main", "HEAD"],
    );
    repo.version_step();
    fs::remove_file(repo.root().join("link")).expect("remove");
    std::os::unix::fs::symlink("CHANGELOG.md", repo.root().join("link")).expect("symlink");
    repo.fails_naming(&["link", "does not write"]);
}

/// A submodule moved to another commit is a change the version step does not
/// make.
#[test]
fn a_version_pr_that_moves_a_submodule_fails() {
    let repo = crate_repo(Some("demo: patch"));
    let head = common::git(repo.root(), &["rev-parse", "HEAD"]);
    fs::create_dir_all(repo.root().join("vendor/sub")).expect("unpopulated submodule");
    let gitlink = |sha: &str| {
        common::git(
            repo.root(),
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("160000,{sha},vendor/sub"),
            ],
        );
    };
    gitlink(&head);
    common::git(repo.root(), &["commit", "-q", "-m", "submodule"]);
    common::git(
        repo.root(),
        &["update-ref", "refs/remotes/origin/main", "HEAD"],
    );
    repo.version_step();
    gitlink(&"1".repeat(40));
    repo.commit();
    repo.fails_naming(&["vendor/sub", "does not write"]);
}

/// The write set comes from the merge-base, so adding a path to
/// `prepare.stage` on the Version PR does not let it change that path.
#[test]
fn a_version_pr_that_widens_its_own_write_set_fails() {
    let repo = crate_repo(Some("demo: patch"));
    repo.version_step();
    common::write_config(
        repo.root(),
        indoc! {"
            prepare:
              stage: [src/lib.rs]
            packages:
              - name: demo
                path: Cargo.toml
        "},
    );
    repo.write("src/lib.rs", "pub fn added() {}");
    repo.commit();
    repo.fails_naming(&["src/lib.rs", ".ctl/ver.yaml"]);
}

/// The pull request that removes `prepare.version_label` has a merge-base
/// config the running verctl refuses. The check is skipped with a notice that
/// names it and the load error, and the config on HEAD still loads.
#[test]
fn a_merge_base_config_that_does_not_load_skips_the_check_with_a_notice() {
    let repo = Repo::new(indoc! {"
        prepare:
          version_label: true
        packages:
          - name: demo
            path: Cargo.toml
    "});
    repo.write(
        "Cargo.toml",
        indoc! {r#"
            [package]
            name = "demo"
            version = "1.0.0"
        "#},
    );
    repo.write(".changeset/fix.md", &fragment("demo: patch"));
    repo.branch();
    common::write_config(
        repo.root(),
        indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
        "},
    );
    repo.commit();
    let notices = repo.check().expect("the check is skipped");
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert!(
        notices[0].contains("content check is skipped"),
        "{notices:?}"
    );
    assert!(
        notices[0].contains("prepare.version_label is removed"),
        "{notices:?}"
    );

    let inputs = content::Inputs::new(
        repo.root().join(config::FILE),
        repo.root().join(".changeset"),
    );
    let candidates = versions::stock_candidates();
    let base = git::MergeBase::read(repo.root(), &candidates)
        .expect("merge-base")
        .expect("a merge-base");
    let found = pull_request::problems(repo.root(), &repo.config(), &inputs, &base, &candidates)
        .expect("the rule is skipped");
    assert!(found.problems.is_empty(), "{found:?}");
    assert_eq!(found.notices.len(), 1, "{found:?}");
    assert!(
        found.notices[0].contains("publish-setting rule is skipped"),
        "{found:?}"
    );
}

#[test]
fn a_path_prepare_stage_names_at_the_merge_base_may_change() {
    let repo = Repo::new(indoc! {"
        prepare:
          stage: [docs/version.txt]
        packages:
          - name: demo
            path: Cargo.toml
    "});
    repo.write(
        "Cargo.toml",
        indoc! {r#"
            [package]
            name = "demo"
            version = "1.0.0"
        "#},
    );
    repo.write("docs/version.txt", "1.0.0");
    repo.write(".changeset/fix.md", &fragment("demo: patch"));
    repo.branch();
    repo.version_step();
    repo.write("docs/version.txt", "1.0.1");
    repo.commit();
    repo.passes();
}

#[test]
fn a_version_pr_that_adds_a_dependency_fails() {
    let repo = crate_repo(Some("demo: patch"));
    repo.version_step();
    let manifest = repo.read("Cargo.toml");
    repo.write("Cargo.toml", &format!(r#"{manifest}serde = "1""#));
    repo.commit();
    repo.fails_naming(&["Cargo.toml", "dependencies.serde"]);
}

#[test]
fn a_requirement_no_pin_explains_fails() {
    let repo = crate_repo(Some("demo: patch"));
    repo.version_step();
    let manifest = repo
        .read("Cargo.toml")
        .replace(r#"foo = "1.0""#, r#"foo = "1.1""#);
    repo.write("Cargo.toml", &manifest);
    repo.commit();
    repo.fails_naming(&["Cargo.toml", "dependencies.foo"]);
}

/// A version moved by hand, with no fragment to plan it, is a change the
/// version step does not make.
#[test]
fn a_version_no_fragment_plans_fails() {
    let repo = crate_repo(None);
    let manifest = repo.read("Cargo.toml").replace("1.0.0", "1.0.1");
    repo.write("Cargo.toml", &manifest);
    repo.commit();
    repo.fails_naming(&["Cargo.toml", "does not write"]);
}

#[test]
fn a_version_pr_without_the_changelog_section_fails() {
    let repo = crate_repo(Some("demo: patch"));
    let changelog = repo.read("CHANGELOG.md");
    repo.version_step();
    repo.write("CHANGELOG.md", &changelog);
    repo.commit();
    repo.fails_naming(&["CHANGELOG.md", "demo 1.0.1"]);
}

#[test]
fn a_version_pr_that_rewrites_an_older_changelog_section_fails() {
    let repo = crate_repo(Some("demo: patch"));
    repo.version_step();
    let changelog = repo.read("CHANGELOG.md").replace("- Start.", "- Started.");
    repo.write("CHANGELOG.md", &changelog);
    repo.commit();
    repo.fails_naming(&["CHANGELOG.md", "line"]);
}

#[test]
fn a_version_pr_that_keeps_a_consumed_fragment_fails() {
    let repo = crate_repo(Some("demo: patch"));
    let pending = repo.read(".changeset/fix.md");
    repo.version_step();
    repo.write(".changeset/fix.md", &pending);
    repo.commit();
    repo.fails_naming(&[".changeset/fix.md", "removes"]);
}

#[test]
fn a_fragment_that_bumps_nothing_is_consumed_on_the_version_pr() {
    let repo = crate_repo(Some("demo: none"));
    repo.version_step();
    repo.commit();
    assert!(!repo.root().join(".changeset/fix.md").exists());
    repo.passes();
}

/// ctl-core's 0.7.0 Version PR: a virtual workspace whose members inherit
/// `workspace.package.version`, three requirements pinned at that version,
/// and four `Cargo.lock` entries that move together.
fn ctl_core_repo() -> Repo {
    let repo = Repo::new(indoc! {r#"
        prepare:
          stage: [Cargo.lock]
        packages:
          - name: ctl-core
            path: Cargo.toml
        patterns:
          layout:
            match: 'ctl-layout = { path = "ctl-layout", version = "={version}" }'
            occurrences: once
            whole_line: true
          term:
            match: 'ctl-term = { path = "ctl-term", version = "={version}" }'
            occurrences: once
            whole_line: true
          text:
            match: 'ctl-text = { path = "ctl-text", version = "={version}" }'
            occurrences: once
            whole_line: true
        pins:
          - file: Cargo.toml
            package: ctl-core
            patterns: [layout, term, text]
    "#});
    repo.write(
        "Cargo.toml",
        indoc! {r#"
            [workspace]
            members = ["ctl-core", "ctl-layout", "ctl-term", "ctl-text"]
            resolver = "3"

            [workspace.package]
            version = "0.6.28"
            edition = "2024"

            [workspace.dependencies]
            ctl-layout = { path = "ctl-layout", version = "=0.6.28" }
            ctl-term = { path = "ctl-term", version = "=0.6.28" }
            ctl-text = { path = "ctl-text", version = "=0.6.28" }
        "#},
    );
    for name in ["ctl-core", "ctl-layout", "ctl-term", "ctl-text"] {
        repo.write(&format!("{name}/Cargo.toml"), &inheriting_member(name));
        repo.write(&format!("{name}/src/lib.rs"), "");
    }
    repo.write(".changeset/release.md", &fragment("ctl-core: minor"));
    repo.copy("ctl-core.before.Cargo.lock", "Cargo.lock");
    repo.branch();
    repo
}

/// A member manifest that inherits the workspace version.
fn inheriting_member(name: &str) -> String {
    formatdoc! {r#"
        [package]
        name = "{name}"
        version.workspace = true
        edition.workspace = true
    "#}
}

#[test]
fn members_that_inherit_the_workspace_version_move_in_the_lock() {
    let repo = ctl_core_repo();
    repo.version_step();
    repo.copy("ctl-core.after.Cargo.lock", "Cargo.lock");
    repo.commit();
    assert!(repo.read("Cargo.toml").contains(r#"version = "=0.7.0""#));
    repo.passes();
}

#[test]
fn a_lock_the_owner_did_not_refresh_fails() {
    let repo = ctl_core_repo();
    repo.version_step();
    repo.commit();
    repo.fails_naming(&["Cargo.lock", "ctl-layout", "0.7.0"]);
}

#[test]
fn a_registry_package_that_moves_in_the_lock_fails() {
    let repo = ctl_core_repo();
    repo.version_step();
    repo.copy("ctl-core.after.Cargo.lock", "Cargo.lock");
    let lock = repo.read("Cargo.lock").replace(
        indoc! {r#"
            name = "anstyle"
            version = "1.0.14"
        "#},
        indoc! {r#"
            name = "anstyle"
            version = "1.0.15"
        "#},
    );
    repo.write("Cargo.lock", &lock);
    repo.commit();
    repo.fails_naming(&["Cargo.lock", "anstyle"]);
}

/// A `Cargo.lock` with one workspace member, no source, per entry.
fn member_lock(members: &[(&str, &str)]) -> String {
    let mut lock = String::from(indoc! {"
        # This file is automatically @generated by Cargo.
        # It is not intended for manual editing.
        version = 4
    "});
    for (name, version) in members {
        lock.push_str(&formatdoc! {r#"

            [[package]]
            name = "{name}"
            version = "{version}"
        "#});
    }
    lock
}

/// unixd: two crates with their own versions in one workspace.
#[test]
fn a_member_that_moves_without_a_planned_bump_fails() {
    let repo = Repo::new(indoc! {"
        packages:
          - name: unixd
            path: unixd/Cargo.toml
          - name: unixd-cache
            path: unixd-cache/Cargo.toml
    "});
    repo.write(
        "Cargo.toml",
        indoc! {r#"
            [workspace]
            members = ["unixd", "unixd-cache"]
            resolver = "3"
        "#},
    );
    for (name, version) in [("unixd", "0.3.0"), ("unixd-cache", "0.2.0")] {
        repo.write(
            &format!("{name}/Cargo.toml"),
            &formatdoc! {r#"
                [package]
                name = "{name}"
                version = "{version}"
                edition = "2024"
            "#},
        );
        repo.write(&format!("{name}/src/lib.rs"), "");
    }
    repo.write(
        "Cargo.lock",
        &member_lock(&[("unixd", "0.3.0"), ("unixd-cache", "0.2.0")]),
    );
    repo.write(".changeset/fix.md", &fragment("unixd: patch"));
    repo.branch();
    repo.version_step();
    repo.write(
        "Cargo.lock",
        &member_lock(&[("unixd", "0.3.1"), ("unixd-cache", "0.2.1")]),
    );
    repo.commit();
    repo.fails_naming(&["Cargo.lock", "unixd-cache"]);
}

/// shrub's 0.7.0 Version PR: eight requirements pinned at the workspace
/// version, and ten `Cargo.lock` entries that inherit it.
#[test]
fn requirements_a_pin_rewrites_and_ten_inheriting_entries_pass() {
    let crates: Vec<String> = (1..=9).map(|at| format!("shrub-{at}")).collect();
    let repo = Repo::new(indoc! {r#"
        prepare:
          stage: [Cargo.lock]
        packages:
          - name: shrub
            path: Cargo.toml
        patterns:
          requirement:
            match: 'version = "={version}", path = "shrub-'
            occurrences: {exactly: 8}
        pins:
          - file: Cargo.toml
            package: shrub
            patterns: [requirement]
    "#});
    let requirements: String = crates
        .iter()
        .take(8)
        .map(|name| {
            formatdoc! {r#"
                {name} = {{ version = "=0.6.2", path = "{name}" }}
            "#}
        })
        .collect();
    let members: Vec<String> = crates.iter().map(|name| format!(r#""{name}""#)).collect();
    repo.write(
        "Cargo.toml",
        &formatdoc! {r#"
            [package]
            name = "shrub"
            version.workspace = true
            edition.workspace = true

            [workspace]
            members = [{members}]
            resolver = "3"

            [workspace.package]
            version = "0.6.2"
            edition = "2024"

            [workspace.dependencies]
            {requirements}"#,
            members = members.join(", "),
        },
    );
    repo.write("src/lib.rs", "");
    for name in &crates {
        repo.write(&format!("{name}/Cargo.toml"), &inheriting_member(name));
        repo.write(&format!("{name}/src/lib.rs"), "");
    }
    let lock = |version: &str| {
        let mut entries = vec![("shrub", version)];
        entries.extend(crates.iter().map(|name| (name.as_str(), version)));
        member_lock(&entries)
    };
    repo.write("Cargo.lock", &lock("0.6.2"));
    repo.write(".changeset/release.md", &fragment("shrub: minor"));
    repo.branch();
    repo.version_step();
    repo.write("Cargo.lock", &lock("0.7.0"));
    repo.commit();
    assert_eq!(repo.read("Cargo.toml").matches(r#""=0.7.0""#).count(), 8);
    repo.passes();
}

/// qctl: the README and the skill pin the version, which moves in each
/// pinned line and nowhere else.
fn qctl_repo() -> Repo {
    let repo = Repo::new(indoc! {r#"
        packages:
          - name: qctl
            path: Cargo.toml
        patterns:
          install:
            match: "cargo install qctl@{version} --locked"
            occurrences: once
          frontmatter:
            match: "version: {version}"
            whole_line: true
            occurrences: once
        pins:
          - file: README.md
            package: qctl
            patterns: [install]
          - file: skills/qctl/SKILL.md
            package: qctl
            patterns: [frontmatter]
    "#});
    repo.write(
        "Cargo.toml",
        indoc! {r#"
            [package]
            name = "qctl"
            version = "0.6.0"
        "#},
    );
    repo.write(
        "README.md",
        indoc! {"
            # qctl

            Run `cargo install qctl@0.6.0 --locked`.
        "},
    );
    repo.write(
        "skills/qctl/SKILL.md",
        indoc! {"
            ---
            name: qctl
            version: 0.6.0
            ---

            Operate the queue.
        "},
    );
    repo.write(".changeset/feat.md", &fragment("qctl: minor"));
    repo.branch();
    repo
}

#[test]
fn pinned_files_on_the_version_pr_pass() {
    let repo = qctl_repo();
    repo.version_step();
    repo.commit();
    assert!(repo.read("README.md").contains("qctl@0.7.0"));
    assert!(repo.read("skills/qctl/SKILL.md").contains("version: 0.7.0"));
    repo.passes();
}

#[test]
fn a_pinned_file_changed_beyond_its_pins_fails() {
    let repo = qctl_repo();
    repo.version_step();
    let readme = repo
        .read("README.md")
        .replace("# qctl", "# qctl, the queue");
    repo.write("README.md", &readme);
    repo.commit();
    repo.fails_naming(&["README.md", "line 1"]);
}

#[test]
fn a_served_file_edited_by_hand_fails() {
    let repo = Repo::new(DEMO_CONFIG);
    repo.write(
        "Cargo.toml",
        indoc! {r#"
            [package]
            name = "demo"
            version = "0.1.0"
        "#},
    );
    repo.write(
        ".ctl/templates/mise.toml.jinja",
        indoc! {r#"
            {%- set path = "examples" -%}
            [tools]
            "cargo:demo" = "{{ versions["demo"] }}"
        "#},
    );
    repo.write("examples/mise.toml", "");
    repo.write(".changeset/fix.md", &fragment("demo: patch"));
    repo.branch();
    repo.version_step();
    let served = repo.read("examples/mise.toml");
    assert!(served.contains(r#""0.1.1""#), "{served}");
    repo.write("examples/mise.toml", &served.replace("0.1.1", "0.1.0"));
    repo.commit();
    repo.fails_naming(&["examples/mise.toml", "mise.toml.jinja"]);
}

const PACKAGE_A: &str = indoc! {"
    packages:
      - name: a
        path: packages/a/package.json
"};

/// A JavaScript workspace whose package `a` moves from 1.0.0 to 1.1.0, with
/// `lock` written at the merge-base as `name`.
fn js_repo(name: &str, lock: &str) -> Repo {
    let repo = Repo::new(PACKAGE_A);
    repo.write(
        "package.json",
        indoc! {r#"
            {
              "name": "demo-root",
              "private": true,
              "workspaces": ["packages/*"]
            }
        "#},
    );
    repo.write(
        "packages/a/package.json",
        indoc! {r#"
            {
              "name": "a",
              "version": "1.0.0",
              "dependencies": { "is-number": "7.0.0" }
            }
        "#},
    );
    repo.write(
        "packages/b/package.json",
        indoc! {r#"
            {
              "name": "b",
              "version": "1.0.0",
              "dependencies": { "a": "workspace:*" }
            }
        "#},
    );
    repo.write(name, lock);
    repo.write(".changeset/feat.md", &fragment("a: minor"));
    repo.branch();
    repo
}

/// A lockfile from `tests/testdata/lockfiles`.
fn testdata(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/testdata/lockfiles")
        .join(name);
    fs::read_to_string(path).expect("testdata")
}

#[test]
fn a_refreshed_bun_lock_passes() {
    let repo = js_repo("bun.lock", &testdata("workspace.before.bun.lock"));
    repo.version_step();
    repo.copy("workspace.after.bun.lock", "bun.lock");
    repo.commit();
    repo.passes();
}

/// bun records no version for the root workspace, so a versioned root
/// `package.json` that the plan bumps leaves `bun.lock` as it was.
#[test]
fn a_bun_lock_needs_no_version_for_the_root_workspace() {
    let repo = Repo::new(indoc! {"
        packages:
          - name: demo-root
            path: package.json
    "});
    repo.write(
        "package.json",
        indoc! {r#"
            {
              "name": "demo-root",
              "version": "1.0.0",
              "private": true
            }
        "#},
    );
    repo.write(
        "bun.lock",
        indoc! {r#"
            {
              "lockfileVersion": 1,
              "workspaces": {
                "": {
                  "name": "demo-root",
                },
              },
              "packages": {
              }
            }
        "#},
    );
    repo.write(".changeset/feat.md", &fragment("demo-root: minor"));
    repo.branch();
    repo.version_step();
    repo.commit();
    repo.passes();
}

#[test]
fn a_stale_bun_lock_fails() {
    let repo = js_repo("bun.lock", &testdata("workspace.before.bun.lock"));
    repo.version_step();
    repo.commit();
    repo.fails_naming(&["bun.lock", "packages/a", "1.1.0"]);
}

/// `prepare.stage` names the lockfile the version step's own command
/// rewrites, such as `yarn.lock`, which verctl cannot read. A path the
/// merge-base stages may change before lockfiles are classified.
#[test]
fn a_staged_lockfile_verctl_cannot_read_may_change() {
    let repo = Repo::new(indoc! {"
        prepare:
          stage: [yarn.lock]
        packages:
          - name: a
            path: packages/a/package.json
    "});
    repo.write(
        "packages/a/package.json",
        indoc! {r#"
            {
              "name": "a",
              "version": "1.0.0"
            }
        "#},
    );
    repo.write(
        "yarn.lock",
        indoc! {"
            # yarn lockfile v1

        "},
    );
    repo.write(".changeset/feat.md", &fragment("a: minor"));
    repo.branch();
    repo.version_step();
    repo.write(
        "yarn.lock",
        indoc! {r#"
            # yarn lockfile v1

            "a@workspace:packages/a":
              version "1.1.0"
        "#},
    );
    repo.commit();
    repo.passes();
}

/// uv writes the normalized project name, so `My_Tool` is `my-tool` in
/// `uv.lock`, and the bump of its own entry is still the version step's.
#[test]
fn a_uv_lock_entry_is_matched_by_its_normalized_name() {
    let repo = Repo::new(indoc! {"
        packages:
          - name: My_Tool
            path: pyproject.toml
    "});
    repo.write(
        "pyproject.toml",
        indoc! {r#"
            [project]
            name = "My_Tool"
            version = "1.0.0"
        "#},
    );
    repo.write(
        "uv.lock",
        indoc! {r#"
            version = 1
            revision = 5

            [[package]]
            name = "my-tool"
            version = "1.0.0"
            source = { virtual = "." }
        "#},
    );
    repo.write(".changeset/feat.md", &fragment("My_Tool: minor"));
    repo.branch();
    repo.version_step();
    repo.write(
        "uv.lock",
        indoc! {r#"
            version = 1
            revision = 5

            [[package]]
            name = "my-tool"
            version = "1.1.0"
            source = { virtual = "." }
        "#},
    );
    repo.commit();
    repo.passes();
}

#[test]
fn a_changed_yarn_lock_fails() {
    let repo = js_repo(
        "yarn.lock",
        indoc! {"
            # yarn lockfile v1

        "},
    );
    repo.version_step();
    repo.write(
        "yarn.lock",
        indoc! {r#"
            # yarn lockfile v1

            "a@workspace:packages/a":
              version "1.1.0"
        "#},
    );
    repo.commit();
    repo.fails_naming(&["yarn.lock", "does not read"]);
}
