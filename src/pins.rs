//! Rewrite collocated tool pins when a package version changes.

use crate::config::{Config, Pin};
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Component, Path, PathBuf};
use toml_edit::{DocumentMut, Item, value};

/// Current declared versions of every configured package.
pub fn current_versions(root: &Path, config: &Config) -> Result<Vec<(String, String)>> {
    let mut versions = Vec::new();
    for spec in &config.packages {
        let path = root.join(&spec.path);
        let driver = spec.resolve(config, root)?;
        let raw = fs::read_to_string(&path).with_context(|| path.display().to_string())?;
        versions.push((spec.name.clone(), driver.read(&raw)?.trim().to_owned()));
    }
    Ok(versions)
}

pub fn write(root: &Path, pins: &[Pin], versions: &[(String, String)]) -> Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    for pin in pins {
        let Some((_, version)) = versions.iter().find(|(name, _)| name == &pin.package) else {
            continue;
        };
        ensure_inside(root, &pin.file)?;
        let path = root.join(&pin.file);
        let raw =
            fs::read_to_string(&path).with_context(|| format!("read pin {}", path.display()))?;
        let mut doc: DocumentMut = raw
            .parse()
            .with_context(|| format!("parse pin {}", path.display()))?;
        let tools = doc
            .get_mut("tools")
            .and_then(Item::as_table_like_mut)
            .with_context(|| format!("{} has no [tools]", path.display()))?;
        let previous = tools
            .get(&pin.tool)
            .and_then(Item::as_str)
            .map(ToOwned::to_owned);
        if previous.is_none() {
            bail!("{} has no tools.{tool}", path.display(), tool = pin.tool);
        }
        tools.insert(&pin.tool, value(version.as_str()));
        let mut body = doc.to_string();
        if let Some(previous) = previous {
            body = rewrite_own_refs(&body, &pin.tool, &previous, version);
        }
        fs::write(&path, body).with_context(|| format!("write pin {}", path.display()))?;
        written.push(path);
    }
    Ok(written)
}

fn ensure_inside(root: &Path, file: &Path) -> Result<()> {
    if file.is_absolute()
        || file
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        bail!("pin file must stay inside the repo: {}", file.display());
    }
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let path = root.join(file);
    if let Ok(meta) = fs::symlink_metadata(&path)
        && meta.file_type().is_symlink()
    {
        bail!("pin file must not be a symlink: {}", path.display());
    }
    let resolved = path.canonicalize().unwrap_or(path);
    if !resolved.starts_with(&root) {
        bail!("pin file must stay inside the repo: {}", file.display());
    }
    Ok(())
}

fn rewrite_own_refs(body: &str, tool: &str, previous: &str, version: &str) -> String {
    let repo = tool.rsplit(':').next().unwrap_or(tool);
    let from = format!("?ref=v{previous}");
    let to = format!("?ref=v{version}");
    let mut out = String::new();
    for line in body.lines() {
        if line.contains(repo) {
            out.push_str(&line.replace(&from, &to));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

#[must_use]
pub fn planned_files(root: &Path, pins: &[Pin], versions: &[(String, String)]) -> Vec<String> {
    pins.iter()
        .filter(|pin| versions.iter().any(|(name, _)| name == &pin.package))
        .map(|pin| root.join(&pin.file).display().to_string())
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use indoc::indoc;
    use tempfile::TempDir;

    fn versions(to: &str) -> Vec<(String, String)> {
        vec![("verctl".into(), to.into())]
    }

    #[test]
    fn updates_the_named_tool_and_keeps_siblings() {
        let root = TempDir::new().unwrap();
        let path = root.path().join("mise.release.toml");
        fs::write(
            &path,
            indoc! {r#"
                [tools]
                leftover = "1"
                "github:victor-software-house/verctl" = "0.0.1"
            "#},
        )
        .unwrap();
        let pins = [Pin {
            file: PathBuf::from("mise.release.toml"),
            tool: "github:victor-software-house/verctl".into(),
            package: "verctl".into(),
        }];
        write(root.path(), &pins, &versions("0.0.2")).unwrap();
        let body = fs::read_to_string(&path).unwrap();
        assert!(body.contains("0.0.2"), "{body}");
        assert!(body.contains("leftover"), "{body}");
        assert!(!body.contains("\"0.0.1\""), "{body}");
    }

    #[test]
    fn skips_when_the_package_is_not_in_the_plan() {
        let root = TempDir::new().unwrap();
        let path = root.path().join("mise.release.toml");
        fs::write(
            &path,
            indoc! {r#"
                [tools]
                "github:victor-software-house/verctl" = "0.0.1"
            "#},
        )
        .unwrap();
        let pins = [Pin {
            file: PathBuf::from("mise.release.toml"),
            tool: "github:victor-software-house/verctl".into(),
            package: "other".into(),
        }];
        assert_eq!(
            write(root.path(), &pins, &versions("0.0.2")).unwrap(),
            Vec::<PathBuf>::new()
        );
        let body = fs::read_to_string(&path).unwrap();
        assert!(body.contains("0.0.1"), "{body}");
    }

    #[test]
    fn rejects_parent_dir_pins() {
        let err = ensure_inside(Path::new("/tmp"), Path::new("../secret")).unwrap_err();
        assert!(format!("{err:#}").contains("inside the repo"), "{err:#}");
    }

    #[test]
    fn rewrites_task_include_ref() {
        let root = TempDir::new().unwrap();
        let path = root.path().join("mise.toml");
        fs::write(
            &path,
            indoc! {r#"
                [tools]
                "github:victor-software-house/verctl" = "0.0.1"
                [task_config]
                includes = ["git::https://github.com/victor-software-house/verctl.git//tasks/ver?ref=v0.0.1"]
            "#},
        )
        .unwrap();
        let pins = [Pin {
            file: PathBuf::from("mise.toml"),
            tool: "github:victor-software-house/verctl".into(),
            package: "verctl".into(),
        }];
        write(root.path(), &pins, &versions("0.0.2")).unwrap();
        let body = fs::read_to_string(&path).unwrap();
        assert!(body.contains("?ref=v0.0.2"), "{body}");
        assert!(!body.contains("verctl.git//tasks/ver?ref=v0.0.1"), "{body}");
    }

    #[test]
    fn leaves_other_repos_refs_alone() {
        let root = TempDir::new().unwrap();
        let path = root.path().join("mise.toml");
        fs::write(
            &path,
            indoc! {r#"
                [tools]
                "github:victor-software-house/verctl" = "0.0.1"
                [task_config]
                includes = ["git::https://example.com/qctl.git//tasks/q?ref=v0.0.1"]
            "#},
        )
        .unwrap();
        let pins = [Pin {
            file: PathBuf::from("mise.toml"),
            tool: "github:victor-software-house/verctl".into(),
            package: "verctl".into(),
        }];
        write(root.path(), &pins, &versions("0.0.2")).unwrap();
        let body = fs::read_to_string(&path).unwrap();
        assert!(body.contains("qctl.git//tasks/q?ref=v0.0.1"), "{body}");
    }

    #[test]
    fn rejects_symlink_pins() {
        let root = TempDir::new().unwrap();
        let target = root.path().join("outside.txt");
        fs::write(&target, "x\n").unwrap();
        let link = root.path().join("mise.release.toml");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let err = ensure_inside(root.path(), Path::new("mise.release.toml")).unwrap_err();
        assert!(format!("{err:#}").contains("symlink"), "{err:#}");
    }
}
