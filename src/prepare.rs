use crate::bump;
use crate::config::Config;
use crate::driver::Driver;
use crate::fragment::{Bump, Fragment};
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanEntry {
    pub name: String,
    pub from: String,
    pub to: String,
    pub bump: Bump,
    pub path: PathBuf,
    pub driver: Driver,
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
        let driver = spec.resolve(config, root)?;
        let path = root.join(&spec.path);
        let raw = std::fs::read_to_string(&path).with_context(|| path.display().to_string())?;
        let from = driver.read(&raw)?.trim().to_owned();
        let to = bump::apply(&from, bump)?;
        plan.push(PlanEntry {
            name: name.to_owned(),
            from,
            to,
            bump,
            path,
            driver,
        });
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
        let updated = entry.driver.write(&raw, &entry.to)?;
        std::fs::write(&entry.path, updated).with_context(|| entry.path.display().to_string())?;
        if let Some(after) = entry.driver.after() {
            follow_up.push(after.to_owned());
        }
    }
    follow_up.sort();
    follow_up.dedup();
    Ok(follow_up)
}
