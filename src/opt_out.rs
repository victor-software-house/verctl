//! Whether a manifest opts its package out of publishing.
//!
//! The stack's own tool reads this setting from the manifest, so verctl reads
//! it there too. A package that opts out releases on GitHub only, with no
//! `publisher: none` beside it. Nothing here follows a symlink or leaves the
//! tree it is given.

use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use toml_edit::{DocumentMut, Item};

use crate::git;

/// How the manifest at `manifest` opts its package out of publishing, in the
/// words an error can repeat. `None` when it publishes, when the file is
/// missing, and for a manifest of another stack.
///
/// A `Cargo.toml` opts out with `package.publish` set to `false` or `[]`, and
/// a `package.json` with `"private": true`. `tree` is the root of the tree the
/// manifest is in, and no lookup leaves it.
///
/// # Errors
///
/// Returns an error when the manifest exists and cannot be read or parsed.
pub(crate) fn opt_out(manifest: &Path, tree: &Path) -> Result<Option<&'static str>> {
    let Some(bytes) = git::read_optional(manifest)? else {
        return Ok(None);
    };
    let text = String::from_utf8(bytes).with_context(|| manifest.display().to_string())?;
    let opted = match manifest.file_name().and_then(|name| name.to_str()) {
        Some("Cargo.toml") => {
            cargo_opts_out(manifest, &text, tree).with_context(|| manifest.display().to_string())?
        }
        Some("package.json") => {
            let json: serde_json::Value =
                serde_json::from_str(&text).with_context(|| manifest.display().to_string())?;
            (json.get("private") == Some(&serde_json::Value::Bool(true)))
                .then_some("package.json sets \"private\": true")
        }
        _ => None,
    };
    Ok(opted)
}

/// How the `Cargo.toml` at `manifest`, holding `text`, sets
/// `package.publish` to `false` or `[]`, or `None` when it does not.
///
/// With `publish.workspace = true`, the value is `workspace.package.publish`
/// of its workspace root. That root is the file `package.workspace` names.
/// Without that key it is the same file when it holds a `[workspace]`, else
/// the nearest ancestor manifest that does, as Cargo finds it.
///
/// The search stays inside `tree` and never passes through a symlink. The
/// merge-base copy and the working tree therefore resolve the same root, and no
/// manifest outside the repository is read.
fn cargo_opts_out(manifest: &Path, text: &str, tree: &Path) -> Result<Option<&'static str>> {
    let document = text.parse::<DocumentMut>()?;
    let Some(package) = document.get("package") else {
        return Ok(None);
    };
    let Some(publish) = package.get("publish") else {
        return Ok(None);
    };
    if publish.get("workspace").and_then(Item::as_bool) != Some(true) {
        return Ok(denies(publish).map(|denial| match denial {
            Denial::False => "Cargo.toml sets publish = false",
            Denial::NoRegistry => "Cargo.toml sets publish = []",
        }));
    }
    let Some(workspace) = workspace_root(manifest, &document, tree)? else {
        return Ok(None);
    };
    Ok(workspace
        .get("workspace")
        .and_then(|table| table.get("package"))
        .and_then(|package| package.get("publish"))
        .and_then(denies)
        .map(|denial| match denial {
            Denial::False => "Cargo.toml inherits publish = false from its workspace",
            Denial::NoRegistry => "Cargo.toml inherits publish = [] from its workspace",
        }))
}

/// The two values of `publish` that turn publishing off, as the
/// [manifest reference] lists them.
///
/// [manifest reference]: https://doc.rust-lang.org/cargo/reference/manifest.html#the-publish-field
enum Denial {
    /// `publish = false`.
    False,
    /// `publish = []`: no registry may receive the package.
    NoRegistry,
}

/// Which value of `publish` turns publishing off, if any.
fn denies(publish: &Item) -> Option<Denial> {
    if publish.as_bool() == Some(false) {
        Some(Denial::False)
    } else if publish.as_array().is_some_and(toml_edit::Array::is_empty) {
        Some(Denial::NoRegistry)
    } else {
        None
    }
}

/// The workspace root manifest of the package at `manifest`, parsed, when
/// one exists inside `tree`. Every path is followed from `tree` without
/// resolving a symlink, in the package's own path and in each candidate's.
/// The merge-base copy holds no symlinks, so a manifest reached through one
/// would let the two trees resolve different roots.
fn workspace_root(
    manifest: &Path,
    document: &DocumentMut,
    tree: &Path,
) -> Result<Option<DocumentMut>> {
    if document.contains_key("workspace") {
        return Ok(Some(document.clone()));
    }
    let parent = manifest.parent().unwrap_or_else(|| Path::new(""));
    let Ok(package) = parent.strip_prefix(tree) else {
        return Ok(None);
    };
    if without_links(tree, package)?.is_none() {
        return Ok(None);
    }
    let named = document
        .get("package")
        .and_then(|package| package.get("workspace"))
        .and_then(Item::as_str);
    let dirs: Vec<PathBuf> = match named {
        Some(path) => vec![package.join(path)],
        None => package.ancestors().skip(1).map(Path::to_path_buf).collect(),
    };
    for dir in dirs {
        let Some(candidate) = without_links(tree, &dir.join("Cargo.toml"))? else {
            continue;
        };
        let Some(bytes) = git::read_optional(&candidate)? else {
            continue;
        };
        let text = String::from_utf8(bytes).with_context(|| candidate.display().to_string())?;
        let parsed = text
            .parse::<DocumentMut>()
            .with_context(|| candidate.display().to_string())?;
        if parsed.contains_key("workspace") {
            return Ok(Some(parsed));
        }
    }
    Ok(None)
}

/// `path`, relative to `tree`, joined onto `tree` one component at a time.
/// `None` when a component is missing or a symlink, or when the path leaves
/// `tree`. A `..` drops a component already checked, so it never resolves
/// through a link.
fn without_links(tree: &Path, path: &Path) -> Result<Option<PathBuf>> {
    let mut resolved = tree.to_path_buf();
    let mut depth = 0_usize;
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if depth == 0 {
                    return Ok(None);
                }
                resolved.pop();
                depth -= 1;
            }
            Component::Normal(name) => {
                resolved.push(name);
                depth += 1;
                match fs::symlink_metadata(&resolved) {
                    Ok(metadata) if metadata.file_type().is_symlink() => return Ok(None),
                    Ok(_) => {}
                    Err(error)
                        if matches!(
                            error.kind(),
                            ErrorKind::NotFound | ErrorKind::NotADirectory
                        ) =>
                    {
                        return Ok(None);
                    }
                    Err(error) => {
                        return Err(error).with_context(|| resolved.display().to_string());
                    }
                }
            }
            Component::RootDir | Component::Prefix(_) => return Ok(None),
        }
    }
    Ok(Some(resolved))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::Path;

    use indoc::{formatdoc, indoc};
    use tempfile::TempDir;

    use super::{cargo_opts_out, opt_out};

    #[test]
    fn a_private_package_json_opts_out() {
        let dir = TempDir::new().expect("tempdir");
        let manifest = dir.path().join("package.json");
        fs::write(&manifest, r#"{"name": "web", "private": true}"#).expect("write");
        assert_eq!(
            opt_out(&manifest, dir.path()).expect("json"),
            Some("package.json sets \"private\": true")
        );
        fs::write(&manifest, r#"{"name": "web"}"#).expect("write");
        assert_eq!(opt_out(&manifest, dir.path()).expect("json"), None);
        assert_eq!(
            opt_out(&dir.path().join("missing/Cargo.toml"), dir.path()).expect("missing"),
            None
        );
    }

    #[test]
    fn publish_false_and_an_empty_list_opt_out() {
        let opts_out = |text: &str| {
            cargo_opts_out(Path::new("Cargo.toml"), text, Path::new("."))
                .expect("toml")
                .is_some()
        };
        assert!(opts_out(indoc! {r#"
            [package]
            name = "a"
            publish = false
        "#}));
        assert!(opts_out(indoc! {r#"
            [package]
            name = "a"
            publish = []
        "#}));
        assert!(!opts_out(indoc! {r#"
            [package]
            name = "a"
            publish = ["crates-io"]
        "#}));
        assert!(opts_out(indoc! {r#"
            [package]
            name = "a"
            publish.workspace = true

            [workspace.package]
            publish = false
        "#}));
    }

    /// A member that inherits `publish` reads it from the workspace root,
    /// as Cargo does.
    #[test]
    fn an_inherited_publish_reads_the_workspace_root() {
        let dir = TempDir::new().expect("tempdir");
        let member = dir.path().join("crates/a/Cargo.toml");
        fs::create_dir_all(member.parent().expect("parent")).expect("dir");
        let root = |line: &str| {
            fs::write(
                dir.path().join("Cargo.toml"),
                formatdoc! {r#"
                    [workspace]
                    members = ["crates/a"]

                    [workspace.package]
                    {line}
                "#},
            )
            .expect("write");
        };
        let text = indoc! {r#"
            [package]
            name = "a"
            publish.workspace = true
        "#};
        root("publish = false");
        assert!(
            cargo_opts_out(&member, text, dir.path())
                .expect("toml")
                .is_some()
        );
        root(r#"edition = "2024""#);
        assert!(
            cargo_opts_out(&member, text, dir.path())
                .expect("toml")
                .is_none()
        );
    }

    /// A manifest above the tree is never read: a workspace there does not
    /// count, and a malformed one does not fail the check.
    #[test]
    fn the_workspace_search_stops_at_the_tree() {
        let dir = TempDir::new().expect("tempdir");
        let tree = dir.path().join("repo");
        let member = tree.join("crates/a/Cargo.toml");
        fs::create_dir_all(member.parent().expect("parent")).expect("dir");
        let text = indoc! {r#"
            [package]
            name = "a"
            publish.workspace = true
        "#};
        let above = dir.path().join("Cargo.toml");
        fs::write(
            &above,
            indoc! {"
                [workspace]

                [workspace.package]
                publish = false
            "},
        )
        .expect("write");
        assert!(
            cargo_opts_out(&member, text, &tree)
                .expect("toml")
                .is_none()
        );
        fs::write(&above, "[workspace").expect("write");
        assert!(
            cargo_opts_out(&member, text, &tree)
                .expect("toml")
                .is_none()
        );
    }

    /// A symlinked `Cargo.toml` above the package is never followed, so a
    /// workspace outside the tree does not count, as in the merge-base copy.
    #[test]
    fn a_symlinked_workspace_manifest_is_skipped() {
        let dir = TempDir::new().expect("tempdir");
        let tree = dir.path().join("repo");
        let member = tree.join("crates/a/Cargo.toml");
        fs::create_dir_all(member.parent().expect("parent")).expect("dir");
        let outside = dir.path().join("outside.toml");
        fs::write(
            &outside,
            indoc! {"
                [workspace]

                [workspace.package]
                publish = false
            "},
        )
        .expect("write");
        symlink(&outside, tree.join("Cargo.toml")).expect("symlink");
        let text = indoc! {r#"
            [package]
            name = "a"
            publish.workspace = true
        "#};
        assert!(
            cargo_opts_out(&member, text, &tree)
                .expect("toml")
                .is_none()
        );
        fs::write(&outside, "[workspace").expect("write");
        assert!(
            cargo_opts_out(&member, text, &tree)
                .expect("toml")
                .is_none()
        );
    }

    /// A workspace reached through a directory symlink does not count. That
    /// holds whether `package.workspace` names the link or the package itself
    /// sits below one. Skipping it matches the merge-base copy, which holds no
    /// links.
    #[test]
    fn a_workspace_behind_a_directory_symlink_is_skipped() {
        let dir = TempDir::new().expect("tempdir");
        let tree = dir.path().join("repo");
        let real = tree.join("real");
        fs::create_dir_all(real.join("a")).expect("dir");
        fs::create_dir_all(tree.join("crates/a")).expect("dir");
        fs::write(
            real.join("Cargo.toml"),
            indoc! {"
                [workspace]

                [workspace.package]
                publish = false
            "},
        )
        .expect("write");
        symlink("real", tree.join("linked")).expect("symlink");
        symlink("../real/a", tree.join("crates/b")).expect("symlink");
        let named = |workspace: &str| {
            formatdoc! {r#"
                [package]
                name = "a"
                workspace = "{workspace}"
                publish.workspace = true
            "#}
        };
        let member = tree.join("crates/a/Cargo.toml");
        assert!(
            cargo_opts_out(&member, &named("../../real"), &tree)
                .expect("toml")
                .is_some()
        );
        assert!(
            cargo_opts_out(&member, &named("../../linked"), &tree)
                .expect("toml")
                .is_none()
        );
        let text = indoc! {r#"
            [package]
            name = "a"
            publish.workspace = true
        "#};
        assert!(
            cargo_opts_out(&real.join("a/Cargo.toml"), text, &tree)
                .expect("toml")
                .is_some()
        );
        assert!(
            cargo_opts_out(&tree.join("crates/b/Cargo.toml"), text, &tree)
                .expect("toml")
                .is_none()
        );
    }
}
