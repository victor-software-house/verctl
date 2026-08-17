use indoc::indoc;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

#[allow(clippy::expect_used)]
pub fn demo_root() -> TempDir {
    let root = TempDir::new().expect("tmp");
    write_demo(root.path());
    root
}

#[allow(clippy::expect_used)]
pub fn write_demo(root: &Path) {
    fs::write(
        root.join("verctl.toml"),
        indoc! {r#"
            [[packages]]
            name = "demo"
            path = "Cargo.toml"
        "#},
    )
    .expect("cfg");
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
