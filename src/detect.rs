//! Infer a lockfile follow-up from the tree. Same signals mise/corepack use.

use std::fs;
use std::path::Path;

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
    walk(dir, "Cargo.lock").then(|| "cargo generate-lockfile".into())
}

fn javascript_follow_up(dir: &Path) -> Option<String> {
    if let Some(name) = package_manager_field(&dir.join("package.json")) {
        return Some(install_command(&name));
    }
    for ancestor in dir.ancestors() {
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
        if ancestor.join(".git").exists() {
            break;
        }
    }
    None
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

fn walk(dir: &Path, file_name: &str) -> bool {
    dir.ancestors().any(|dir| dir.join(file_name).is_file())
}
