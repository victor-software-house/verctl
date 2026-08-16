use crate::bump;
use crate::config::{Config, ManifestKind};
use crate::fragment::{Bump, Fragment};
use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanEntry {
    pub name: String,
    pub from: String,
    pub to: String,
    pub bump: Bump,
    pub path: std::path::PathBuf,
    pub kind: ManifestKind,
}

pub fn plan(config: &Config, fragments: &[Fragment], root: &Path) -> Result<Vec<PlanEntry>> {
    let mut by_name: BTreeMap<&str, Bump> = BTreeMap::new();
    for fragment in fragments {
        for package in &fragment.packages {
            let bump = by_name.entry(package.name.as_str()).or_insert(Bump::None);
            if package.bump > *bump {
                *bump = package.bump;
            }
        }
    }
    let mut plan = Vec::new();
    for (name, bump) in by_name {
        if bump == Bump::None {
            continue;
        }
        let spec = config.find(name)?;
        let kind = spec.kind()?;
        let path = root.join(&spec.path);
        let raw = std::fs::read_to_string(&path).with_context(|| path.display().to_string())?;
        let from = bump::read_version(kind, &raw)?;
        let to = bump::apply(&from, bump)?;
        plan.push(PlanEntry {
            name: name.to_owned(),
            from,
            to,
            bump,
            path,
            kind,
        });
    }
    if plan.is_empty() {
        bail!("no version-changing fragments");
    }
    Ok(plan)
}

pub fn apply_plan(entries: &[PlanEntry]) -> Result<Vec<String>> {
    let mut follow_up = Vec::new();
    for entry in entries {
        if entry.from == entry.to {
            continue;
        }
        let raw = std::fs::read_to_string(&entry.path)
            .with_context(|| entry.path.display().to_string())?;
        let updated = bump::write_version(entry.kind, &raw, &entry.to)?;
        std::fs::write(&entry.path, updated).with_context(|| entry.path.display().to_string())?;
        match entry.kind {
            ManifestKind::Cargo => follow_up.push("cargo generate-lockfile".into()),
            ManifestKind::Npm => follow_up.push("bun install".into()),
        }
    }
    follow_up.sort();
    follow_up.dedup();
    Ok(follow_up)
}
