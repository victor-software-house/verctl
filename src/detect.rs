//! Infer a lockfile follow-up from the tree. Same signals mise/corepack use.

use std::fs;
use std::path::{Path, PathBuf};

use crate::git;

/// The command that owns a manifest's lockfile: what to run, where, and the
/// lockfile it rewrites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owner {
    /// The command, such as `cargo update --workspace`.
    pub argv: Vec<String>,
    /// Where it runs: the directory holding the lockfile, or the manifest's
    /// when no lockfile exists yet.
    pub dir: PathBuf,
    /// The lockfile the command rewrites, when the tree has one.
    pub lockfile: Option<PathBuf>,
}

impl Owner {
    /// The argv joined by spaces, as a report and a driver's `after` show it.
    #[must_use]
    pub fn command(&self) -> String {
        self.argv.join(" ")
    }

    fn new(command: &str, dir: &Path, lockfile: Option<PathBuf>) -> Self {
        let dir = lockfile
            .as_deref()
            .and_then(Path::parent)
            .map_or_else(|| dir.to_path_buf(), Path::to_path_buf);
        Self {
            argv: command.split_whitespace().map(str::to_owned).collect(),
            dir,
            lockfile,
        }
    }
}

/// The command that refreshes the lockfile after `manifest`'s version moves,
/// when the tree has one.
#[must_use]
pub fn follow_up(manifest: &Path) -> Option<String> {
    owner(manifest).map(|owner| owner.command())
}

/// The command that owns `manifest`'s lockfile, found by the signals of
/// [`follow_up`].
#[must_use]
pub fn owner(manifest: &Path) -> Option<Owner> {
    let dir = manifest.parent().unwrap_or(manifest);
    let file_name = manifest.file_name()?.to_str()?;
    match file_name {
        "Cargo.toml" => cargo_owner(dir),
        "package.json" => javascript_owner(dir),
        "pyproject.toml" => python_owner(dir),
        _ => None,
    }
}

fn cargo_owner(dir: &Path) -> Option<Owner> {
    // Not `generate-lockfile`: that re-resolves every dependency, so a bump
    // would carry unrelated moves. Only this package's version went stale.
    let lockfile = find(dir, "Cargo.lock")?;
    Some(Owner::new("cargo update --workspace", dir, Some(lockfile)))
}

/// `uv.lock` records the project's own version, so a bump leaves it stale.
/// `uv lock` keeps every locked dependency it can ([uv: upgrading locked
/// package versions]), so only that version moves.
///
/// [uv: upgrading locked package versions]: https://docs.astral.sh/uv/concepts/projects/sync/#upgrading-locked-package-versions
fn python_owner(dir: &Path) -> Option<Owner> {
    let lockfile = find(dir, "uv.lock")?;
    Some(Owner::new("uv lock", dir, Some(lockfile)))
}

/// The lockfiles each JavaScript package manager writes, in the order the
/// tree is searched.
const JAVASCRIPT_LOCKFILES: [(&str, &str); 6] = [
    ("bun", "bun.lock"),
    ("bun", "bun.lockb"),
    ("pnpm", "pnpm-lock.yaml"),
    ("yarn", "yarn.lock"),
    ("npm", "package-lock.json"),
    ("npm", "npm-shrinkwrap.json"),
];

fn javascript_owner(dir: &Path) -> Option<Owner> {
    if let Some(name) = package_manager_field(&dir.join("package.json")) {
        let lockfile = JAVASCRIPT_LOCKFILES
            .iter()
            .filter(|(manager, _)| *manager == name)
            .find_map(|(_, file_name)| find(dir, file_name));
        return Some(Owner::new(&install_command(&name), dir, lockfile));
    }
    for ancestor in scope(dir) {
        for (manager, file_name) in JAVASCRIPT_LOCKFILES {
            let lockfile = ancestor.join(file_name);
            if lockfile.is_file() {
                return Some(Owner::new(&install_command(manager), dir, Some(lockfile)));
            }
        }
    }
    None
}

/// `dir` and every ancestor up to the working tree that holds it.
///
/// The repository root is the ceiling: a lockfile above it belongs to
/// some other project that happens to contain this one on disk. Without
/// a repository the search never leaves `dir` at all, so a lockfile in a
/// parent directory is never matched to this manifest.
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

/// The nearest `file_name` in [`scope`].
fn find(dir: &Path, file_name: &str) -> Option<PathBuf> {
    scope(dir)
        .into_iter()
        .map(|ancestor| ancestor.join(file_name))
        .find(|path| path.is_file())
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
    use std::fs;
    use std::path::Path;

    use tempfile::TempDir;

    use super::follow_up;
    use crate::test_git;

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
        test_git::init(dir);
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
    fn a_workspace_member_runs_its_owner_beside_the_lock() {
        let root = tree(&["Cargo.lock", "crates/member/Cargo.toml"]);
        repo_at(root.path());
        let owner = super::owner(&root.path().join("crates/member/Cargo.toml")).unwrap();
        let top = root.path().canonicalize().unwrap();
        assert_eq!(owner.dir, top);
        assert_eq!(owner.lockfile, Some(top.join("Cargo.lock")));
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
    fn a_uv_lock_at_the_repository_root_serves_a_python_package() {
        let root = tree(&["uv.lock", "tools/cli/pyproject.toml"]);
        repo_at(root.path());
        assert_eq!(
            follow_up(&root.path().join("tools/cli/pyproject.toml")),
            Some("uv lock".into())
        );
    }

    #[test]
    fn a_python_package_without_a_uv_lock_has_no_follow_up() {
        let root = tree(&["pyproject.toml"]);
        assert_eq!(follow_up(&root.path().join("pyproject.toml")), None);
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
