//! The hidden `--usage-spec` flag and the mount line of a served task.

mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};
use std::{env, fs, iter};

use indoc::indoc;

fn crate_file(relative: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(relative))
        .unwrap_or_else(|error| panic!("read {relative}: {error}"))
}

fn verctl(args: &[&str]) -> Output {
    common::verctl()
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("spawn verctl: {error}"))
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn usage_spec_is_the_mounted_ver_grammar() {
    let output = verctl(&["--usage-spec=ver"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let spec = stdout(&output);
    assert!(spec.lines().any(|line| line == "name ver"), "{spec}");
    assert!(spec.lines().any(|line| line == "bin ver"), "{spec}");
}

#[test]
fn usage_spec_bare_defaults_to_ver() {
    let output = verctl(&["--usage-spec"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), stdout(&verctl(&["--usage-spec=ver"])));
}

#[test]
fn template_mount_is_ctl_core() {
    let template = crate_file(".ctl/templates/ver.jinja");
    let directives = ctl_core::mount_directives("ver");
    assert!(
        template.contains(&directives),
        "template must carry ctl_core::mount_directives(\"ver\"):\n{directives}\n\n{template}"
    );
}

/// In prefix output mode, mise relays a task's output itself unless the task
/// is `raw`. Mise uses that mode for several tasks at once, or when
/// `MISE_TASK_OUTPUT=prefix` is set.
///
/// The relay appends a Select Graphic Rendition (SGR) reset to every line
/// whenever mise's own colour is on. `CLICOLOR_FORCE=1` turns that on even into
/// a pipe. A single task defaults to `interleave`, which passes output through.
/// The test therefore forces prefix mode, and without `raw` it fails.
///
/// The served task is rendered from its template. It runs in a scrubbed
/// environment, with a stub `verctl` on `PATH`. Neither the caller's mise
/// configuration nor a download is involved.
#[test]
fn the_served_task_pipes_json_without_an_escape_byte() {
    let mut environment = minijinja::Environment::new();
    environment.set_keep_trailing_newline(true);
    let task = environment
        .render_str(
            &crate_file(".ctl/templates/ver.jinja"),
            minijinja::context! { versions => minijinja::context! { verctl => "0.0.0" } },
        )
        .unwrap_or_else(|error| panic!("render ver.jinja: {error}"));
    let root = tempfile::TempDir::new().unwrap_or_else(|error| panic!("tempdir: {error}"));
    let write = |relative: &str, body: &str| {
        let path = root.path().join(relative);
        fs::create_dir_all(path.parent().unwrap_or(root.path()))
            .unwrap_or_else(|error| panic!("mkdir for {relative}: {error}"));
        fs::write(&path, body).unwrap_or_else(|error| panic!("write {relative}: {error}"));
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
            .unwrap_or_else(|error| panic!("chmod {relative}: {error}"));
    };
    write(
        "mise.toml",
        indoc! {r#"
            [task_config]
            includes = ["tasks"]
        "#},
    );
    write("tasks/ver", &task);
    write(
        "bin/verctl",
        indoc! {r#"
            #!/bin/sh
            printf '{"ok":true}\n'
        "#},
    );
    let path = env::join_paths(
        iter::once(root.path().join("bin"))
            .chain(env::split_paths(&env::var_os("PATH").unwrap_or_default())),
    )
    .unwrap_or_else(|error| panic!("PATH: {error}"));
    let output = Command::new("mise")
        .args(["run", "ver", "status", "--format", "json"])
        .current_dir(root.path())
        .env_clear()
        .env("PATH", path)
        .env("HOME", env::var_os("HOME").unwrap_or_default())
        .env("MISE_CONFIG_DIR", root.path().join("mise-config"))
        .env("MISE_TRUSTED_CONFIG_PATHS", root.path())
        .env("MISE_DISABLE_TOOLS", "github:victor-software-house/verctl")
        .env("MISE_TASK_OUTPUT", "prefix")
        .env("CLICOLOR_FORCE", "1")
        .env("FORCE_COLOR", "3")
        .env("COLORTERM", "truecolor")
        .output()
        .unwrap_or_else(|error| panic!("this test needs mise on PATH: {error}"));
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "{\"ok\":true}\n",
        "stdout must be exactly what the CLI printed"
    );
}

/// The usage spec lists none of the verbs 0.8.0 renamed, so neither help nor
/// completion offers them.
#[test]
fn usage_spec_lists_no_renamed_verb() {
    let output = verctl(&["--usage-spec=ver"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let spec = stdout(&output);
    for removed in ["prepare", "pin", "publish", "assets"] {
        assert!(
            !spec
                .lines()
                .any(|line| line.trim_start().starts_with(&format!("cmd {removed} "))),
            "cmd {removed}:\n{spec}"
        );
    }
}
