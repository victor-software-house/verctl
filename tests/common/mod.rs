#![allow(
    dead_code,
    clippy::expect_used,
    reason = "each test binary uses its own subset of these fixture helpers, which fail fast"
)]

use indoc::indoc;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Write a repo's declarations where verctl looks for them, and hand back the
/// path so a test can pass it to `-c` or `Config::load` without spelling the
/// layout again.
pub(crate) fn write_config(root: &Path, yaml: &str) -> PathBuf {
    let path = root.join(verctl::config::FILE);
    fs::create_dir_all(path.parent().expect("parent")).expect("dir");
    fs::write(&path, yaml).expect("cfg");
    path
}

pub(crate) fn demo_root() -> TempDir {
    let root = TempDir::new().expect("tmp");
    write_demo(root.path());
    root
}

pub(crate) fn write_demo(root: &Path) {
    write_config(
        root,
        indoc! {"
            packages:
              - name: demo
                path: Cargo.toml
        "},
    );
    fs::write(
        root.join("Cargo.toml"),
        indoc! {r#"
            [package]
            name = "demo"
            version = "1.0.0"
        "#},
    )
    .expect("cargo");
    let changes = root.join(".changeset");
    fs::create_dir_all(&changes).expect("dir");
    fs::write(
        changes.join("bump.md"),
        indoc! {"
            ---
            demo: patch
            ---

            Patch.
        "},
    )
    .expect("frag");
}
