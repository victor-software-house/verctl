//! Infer a lockfile follow-up from the tree. Same signals mise/corepack use.

use crate::git;
use std::fs;
use std::path::{Path, PathBuf};

#[must_use]
pub fn follow_up(manifest: &Path) -> Option<String> {
    let dir = manifest.parent().unwrap_or(manifest);
    let file_name = manifest.file_name()?.to_str()?;
    match file_name {
        "Cargo.toml" => cargo_follow_up(dir),
        "package.json" => javascript_follow_up(dir),
        _ => None,
    }
}

fn cargo_follow_up(dir: &Path) -> Option<String> {
    // Not `generate-lockfile`: that re-resolves every dependency, so a bump
    // would carry unrelated moves. Only this package's version went stale.
    holds(dir, "Cargo.lock").then(|| "cargo update --workspace".into())
}

fn javascript_follow_up(dir: &Path) -> Option<String> {
    if let Some(name) = package_manager_field(&dir.join("package.json")) {
        return Some(install_command(&name));
    }
    for ancestor in scope(dir) {
        if ancestor.join("bun.lock").is_file() || ancestor.join("bun.lockb").is_file() {
            return Some(install_command("bun"));
        }
        if ancestor.join("pnpm-lock.yaml").is_file() {
            return Some(install_command("pnpm"));
        }
        if ancestor.join("yarn.lock").is_file() {
            return Some(install_command("yarn"));
        }
        if ancestor.join("package-lock.json").is_file()
            || ancestor.join("npm-shrinkwrap.json").is_file()
        {
            return Some(install_command("npm"));
        }
    }
    None
}

/// `dir` and every ancestor up to the working tree that holds it.
///
/// The repository root is the ceiling: a lockfile above it belongs to
/// some other project that happens to contain this one on disk. Without
/// a repository the search never leaves `dir` at all, so a stray
/// lockfile in a parent directory cannot claim this manifest.
fn scope(dir: &Path) -> Vec<PathBuf> {
    let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    let ceiling = git::workdir_covering(&dir);
    let mut dirs = Vec::new();
    for ancestor in dir.ancestors() {
        dirs.push(ancestor.to_path_buf());
        if ceiling.as_deref().is_none_or(|ceiling| ancestor == ceiling) {
            break;
        }
    }
    dirs
}

fn holds(dir: &Path, file_name: &str) -> bool {
    scope(dir)
        .iter()
        .any(|ancestor| ancestor.join(file_name).is_file())
}

fn package_manager_field(package_json: &Path) -> Option<String> {
    let raw = fs::read_to_string(package_json).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let field = value.get("packageManager")?.as_str()?;
    let name = field.split('@').next()?.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

fn install_command(manager: &str) -> String {
    match manager {
        "bun" => "bun install".into(),
        "pnpm" => "pnpm install".into(),
        "yarn" => "yarn install".into(),
        other => format!("{other} install"),
    }
}

#[cfg(test)]
mod tests {
    use super::follow_up;
    use git2::Repository;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    /// `files` are workspace-relative; parents are created.
    fn tree(files: &[&str]) -> TempDir {
        let root = TempDir::new().unwrap();
        for rel in files {
            let path = root.path().join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, "").unwrap();
        }
        root
    }

    fn repo_at(dir: &Path) {
        Repository::init(dir).unwrap();
    }

    #[test]
    fn a_cargo_lock_beside_the_manifest_asks_for_the_narrowest_refresh() {
        let root = tree(&["Cargo.toml", "Cargo.lock"]);
        assert_eq!(
            follow_up(&root.path().join("Cargo.toml")),
            Some("cargo update --workspace".into())
        );
    }

    #[test]
    fn a_workspace_member_finds_the_lock_at_the_repository_root() {
        let root = tree(&["Cargo.lock", "crates/member/Cargo.toml"]);
        repo_at(root.path());
        assert_eq!(
            follow_up(&root.path().join("crates/member/Cargo.toml")),
            Some("cargo update --workspace".into())
        );
    }

    #[test]
    fn a_cargo_lock_above_the_repository_is_not_ours() {
        let root = tree(&["Cargo.lock", "checkout/Cargo.toml"]);
        repo_at(&root.path().join("checkout"));
        assert_eq!(follow_up(&root.path().join("checkout/Cargo.toml")), None);
    }

    #[test]
    fn without_a_repository_the_search_stays_in_the_directory() {
        let root = tree(&["Cargo.lock", "member/Cargo.toml"]);
        assert_eq!(follow_up(&root.path().join("member/Cargo.toml")), None);
    }

    #[test]
    fn a_bun_lock_at_the_repository_root_serves_a_package() {
        let root = tree(&["bun.lock", "packages/pkg/package.json"]);
        repo_at(root.path());
        assert_eq!(
            follow_up(&root.path().join("packages/pkg/package.json")),
            Some("bun install".into())
        );
    }

    #[test]
    fn a_bun_lock_above_the_repository_is_not_ours() {
        let root = tree(&["bun.lock", "checkout/package.json"]);
        repo_at(&root.path().join("checkout"));
        assert_eq!(follow_up(&root.path().join("checkout/package.json")), None);
    }

    #[test]
    fn the_package_manager_field_outranks_the_lockfile() {
        let root = TempDir::new().unwrap();
        fs::write(root.path().join("bun.lock"), "").unwrap();
        fs::write(
            root.path().join("package.json"),
            r#"{"packageManager":"pnpm@10.0.0"}"#,
        )
        .unwrap();
        assert_eq!(
            follow_up(&root.path().join("package.json")),
            Some("pnpm install".into())
        );
    }
}
