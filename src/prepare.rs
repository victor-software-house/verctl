use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::bump;
use crate::config::Config;
use crate::driver::Driver;
use crate::fragment::{Bump, Fragment};

/// One package's planned version change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanEntry {
    /// Package name.
    pub name: String,
    /// Version now.
    pub from: String,
    /// Version after the bump.
    pub to: String,
    /// Largest bump among the fragments naming it.
    pub bump: Bump,
    /// Manifest path.
    pub path: PathBuf,
    /// Driver that reads and writes the manifest's version.
    pub driver: Driver,
}

/// Fold the fragments into one bump per package, and leave out each package
/// whose version stays.
///
/// A package no fragment bumps still moves in two cases, because its
/// manifest and the config say so. Only a version that parses as semver and
/// holds a prerelease is looked at, so a package whose version is not semver
/// never fails the plan unless a fragment names it. The cases are: its
/// version is a prerelease and
/// `prerelease` is omitted (the stable release), or its version is a
/// prerelease of another tag than `prerelease` (the first candidate of the
/// new tag). Such an entry has [`Bump::None`] and a `to` that differs from
/// its `from`.
///
/// # Errors
///
/// Returns an error when:
///
/// - a fragment names a package the config does not declare,
/// - a package's driver or manifest cannot be resolved or read,
/// - its current version cannot be read, or
/// - [`crate::bump::apply_with`] refuses the bump.
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
    for spec in &config.packages {
        by_name.entry(spec.name.as_str()).or_insert(Bump::None);
    }
    let mut plan = Vec::new();
    for (name, bump) in by_name {
        let spec = config.find(name)?;
        let current = spec.current(config, root)?;
        if bump == Bump::None
            && !semver::Version::parse(&current.version)
                .is_ok_and(|version| !version.pre.is_empty())
        {
            continue;
        }
        let to = bump::apply_with(
            &current.version,
            bump,
            spec.first_minor,
            spec.prerelease.as_ref(),
        )
        .with_context(|| format!("package {name}"))?;
        if to == current.version {
            continue;
        }
        current
            .driver
            .accepts(&current.raw, &to)
            .with_context(|| format!("package {name}: {}", current.path.display()))?;
        plan.push(PlanEntry {
            name: name.to_owned(),
            from: current.version,
            to,
            bump,
            path: current.path,
            driver: current.driver,
        });
    }
    Ok(plan)
}

/// Write each new version.
///
/// # Errors
///
/// Returns an error when a manifest cannot be read or written, or when its
/// driver cannot write the new version. Manifests written before the failing
/// entry stay changed.
pub fn apply_plan(entries: &[PlanEntry]) -> Result<()> {
    for entry in entries {
        if entry.from == entry.to {
            continue;
        }
        let raw = entry.driver.source(&entry.path)?;
        let updated = entry.driver.write(&raw, &entry.to)?;
        std::fs::write(&entry.path, updated).with_context(|| entry.path.display().to_string())?;
    }
    Ok(())
}

/// A command the version step runs once the files it writes are written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// The command.
    pub argv: Vec<String>,
    /// Where it runs.
    pub dir: PathBuf,
    /// The lockfile it rewrites, which the Version PR commits. Only a detected
    /// owner's command sets it. A command a repository declares stages what it
    /// writes through `prepare.stage`.
    pub lockfile: Option<PathBuf>,
}

impl Step {
    /// The argv joined by spaces, as the report's `after` lists it.
    #[must_use]
    pub fn command(&self) -> String {
        self.argv.join(" ")
    }
}

/// The commands the version step runs after the bumps, each once: the owner
/// of each bumped manifest's lockfile ([`crate::detect::owner`]), or the
/// package's own `after`, then `prepare.after`.
///
/// A lockfile refresh belongs to the version step, so a lockfile on the
/// Version PR comes from its owner and never from a hand edit. A package's
/// own `after` and `prepare.after` run in `cwd`, as `prepare.after` always
/// has; a command already listed for the same directory runs once.
#[must_use]
pub fn steps(entries: &[PlanEntry], prepare_after: Option<&[String]>, cwd: &Path) -> Vec<Step> {
    let mut steps: Vec<Step> = Vec::new();
    let mut push = |step: Step| {
        if !steps
            .iter()
            .any(|ran| ran.argv == step.argv && ran.dir == step.dir)
        {
            steps.push(step);
        }
    };
    for entry in entries {
        if entry.from == entry.to {
            continue;
        }
        let Some(after) = entry.driver.after() else {
            continue;
        };
        match crate::detect::owner(&entry.path) {
            Some(owner) if owner.command() == after => push(Step {
                argv: owner.argv,
                dir: owner.dir,
                lockfile: owner.lockfile,
            }),
            Some(_) | None => push(Step {
                argv: after.split_whitespace().map(str::to_owned).collect(),
                dir: cwd.to_path_buf(),
                lockfile: None,
            }),
        }
    }
    if let Some(argv) = prepare_after {
        push(Step {
            argv: argv.to_vec(),
            dir: cwd.to_path_buf(),
            lockfile: None,
        });
    }
    steps
}

/// Run each step in order, streaming its output to stderr.
///
/// # Errors
///
/// Returns an error naming the first command that fails or runs longer than
/// five minutes. Steps before it have run.
pub fn run_steps(steps: &[Step]) -> Result<()> {
    for step in steps {
        crate::process::run_inherit_in(
            &step.argv,
            Some(&step.dir),
            std::time::Duration::from_mins(5),
        )
        .with_context(|| format!("version step `{}`", step.command()))?;
    }
    Ok(())
}
