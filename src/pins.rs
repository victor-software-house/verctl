//! Rewrite collocated tool pins when a package version changes.

use crate::config::Pin;
use crate::prepare::PlanEntry;
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};
use toml_edit::{DocumentMut, Item, value};

pub fn write(root: &Path, pins: &[Pin], plan: &[PlanEntry]) -> Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    for pin in pins {
        let Some(entry) = plan.iter().find(|entry| entry.name == pin.package) else {
            continue;
        };
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
        if tools.get(&pin.tool).is_none() {
            bail!("{} has no tools.{tool}", path.display(), tool = pin.tool);
        }
        tools.insert(&pin.tool, value(entry.to.as_str()));
        fs::write(&path, doc.to_string())
            .with_context(|| format!("write pin {}", path.display()))?;
        written.push(path);
    }
    Ok(written)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::driver::{Driver, Format};
    use crate::fragment::Bump;
    use indoc::indoc;
    use tempfile::TempDir;

    fn plan(to: &str) -> Vec<PlanEntry> {
        vec![PlanEntry {
            name: "verctl".into(),
            from: "0.0.1".into(),
            to: to.into(),
            bump: Bump::Patch,
            path: PathBuf::from("Cargo.toml"),
            driver: Driver::Path {
                format: Format::Toml,
                keys: vec!["package.version".into()],
                after: None,
            },
        }]
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
        write(root.path(), &pins, &plan("0.0.2")).unwrap();
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
            write(root.path(), &pins, &plan("0.0.2")).unwrap(),
            Vec::<PathBuf>::new()
        );
        let body = fs::read_to_string(&path).unwrap();
        assert!(body.contains("0.0.1"), "{body}");
    }
}
