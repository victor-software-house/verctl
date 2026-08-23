//! Hidden `--usage-spec` and the served-task mount line.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn crate_file(relative: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(relative))
        .unwrap_or_else(|error| panic!("read {relative}: {error}"))
}

fn verctl(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_verctl"))
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
fn template_mount_line_is_ctl_core() {
    let template = crate_file(".ctl/templates/ver.jinja");
    let line = template
        .lines()
        .find(|line| line.starts_with("#USAGE mount"))
        .unwrap_or_else(|| panic!("no #USAGE mount in template:\n{template}"));
    assert_eq!(line, ctl_core::mount_line("ver"));
}
