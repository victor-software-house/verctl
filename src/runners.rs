//! Machines: declared once as `[runners.NAME]`, named by every job.
//!
//! A runner is one machine and `labels` is how GitHub finds it — every label
//! at once, so `["self-hosted", "linux", "x64"]` is a single machine carrying
//! three labels. Nothing here resolves a *label*: verctl cannot know which
//! machines carry which, so labels pass through untouched.
//!
//! A runner *name*, by contrast, resolves here or fails here. It never reaches
//! GitHub — only its labels do.

use crate::config::{Config, Runner};
use anyhow::{Result, bail, ensure};
use std::collections::{BTreeMap, BTreeSet};

/// A machine after resolution: the name it was declared under, and the labels
/// that go straight into `runs-on:`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub name: String,
    pub labels: Vec<String>,
}

/// Check every declared machine before any job names one, so a broken
/// `[runners.NAME]` fails even when nothing uses it yet.
pub fn declared(config: &Config) -> Result<()> {
    for (name, runner) in &config.runners {
        validate(name, runner)?;
    }
    Ok(())
}

fn validate(name: &str, runner: &Runner) -> Result<()> {
    ensure!(
        !runner.labels.is_empty(),
        "runner {name:?}: labels must name at least one label"
    );
    let mut seen = BTreeSet::new();
    for label in &runner.labels {
        ensure!(
            !label.trim().is_empty(),
            "runner {name:?}: labels has an empty label"
        );
        ensure!(
            seen.insert(label.as_str()),
            "runner {name:?}: label {label:?} is repeated"
        );
    }
    Ok(())
}

/// The machines a job named. `None` (field omitted) takes `default_labels`
/// under the name `default_name`.
pub fn of(
    config: &Config,
    kind: &str,
    job: &str,
    named: Option<&Vec<String>>,
    default_name: &str,
    default_labels: &[&str],
) -> Result<Vec<Machine>> {
    let Some(named) = named else {
        return Ok(vec![Machine {
            name: default_name.to_owned(),
            labels: default_labels
                .iter()
                .map(|label| (*label).to_owned())
                .collect(),
        }]);
    };
    ensure!(
        !named.is_empty(),
        "{kind} {job:?}: runners must name at least one machine"
    );
    let mut machines = Vec::new();
    let mut seen = BTreeSet::new();
    for name in named {
        ensure!(
            seen.insert(name.as_str()),
            "{kind} {job:?}: machine {name:?} is named twice"
        );
        machines.push(one(config, kind, job, name)?);
    }
    Ok(machines)
}

/// One machine by name. An undeclared name cannot be guessed at: verctl would
/// be inventing a label GitHub has never seen.
pub fn one(config: &Config, kind: &str, job: &str, name: &str) -> Result<Machine> {
    let Some(runner) = config.runners.get(name) else {
        bail!(
            "{kind} {job:?}: machine {name:?} is not declared in [runners] ({})",
            declared_names(&config.runners)
        );
    };
    validate(name, runner)?;
    Ok(Machine {
        name: name.to_owned(),
        labels: runner.labels.clone(),
    })
}

fn declared_names(runners: &BTreeMap<String, Runner>) -> String {
    if runners.is_empty() {
        return "none declared".to_owned();
    }
    format!(
        "declared: {}",
        runners.keys().cloned().collect::<Vec<_>>().join(", ")
    )
}

/// The check name. One machine needs no disambiguation; several would land as
/// same-named checks, each hiding the others.
#[must_use]
pub fn check_name(job: &str, machine: &str, fanned_out: bool) -> String {
    if fanned_out {
        format!("{job} ({machine})")
    } else {
        job.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::{Machine, check_name, declared, of, one};
    use crate::config::Config;

    fn load(toml: &str) -> Config {
        toml::from_str(toml).expect("parse")
    }

    const REGISTRY: &str = indoc::indoc! {r#"
        [[packages]]
        name = "verctl"
        path = "Cargo.toml"

        [runners.linux]
        labels = ["ubuntu-latest"]

        [runners.big]
        labels = ["self-hosted", "linux", "x64"]
    "#};

    fn named(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn omitted_takes_the_default_machine() {
        let machines = of(
            &load(REGISTRY),
            "ci job",
            "verify",
            None,
            "hosted",
            &["ubuntu-latest"],
        )
        .unwrap();
        assert_eq!(
            machines,
            [Machine {
                name: "hosted".to_owned(),
                labels: vec!["ubuntu-latest".to_owned()],
            }]
        );
    }

    #[test]
    fn labels_reach_the_caller_verbatim_and_in_order() {
        let machines = of(
            &load(REGISTRY),
            "ci job",
            "verify",
            Some(&named(&["big"])),
            "hosted",
            &[],
        )
        .unwrap();
        assert_eq!(machines[0].labels, ["self-hosted", "linux", "x64"]);
    }

    #[test]
    fn one_machine_with_three_labels_is_one_check() {
        let machines = of(
            &load(REGISTRY),
            "ci job",
            "verify",
            Some(&named(&["big"])),
            "hosted",
            &[],
        )
        .unwrap();
        assert_eq!(machines.len(), 1);
    }

    #[test]
    fn two_machines_are_two_checks() {
        let machines = of(
            &load(REGISTRY),
            "ci job",
            "verify",
            Some(&named(&["linux", "big"])),
            "hosted",
            &[],
        )
        .unwrap();
        assert_eq!(machines.len(), 2);
    }

    #[test]
    fn an_undeclared_machine_names_the_declared_ones() {
        let err = one(&load(REGISTRY), "ci job", "verify", "macos").unwrap_err();
        let text = format!("{err:#}");
        assert!(text.contains("not declared in [runners]"), "{text}");
        assert!(text.contains("declared: big, linux"), "{text}");
    }

    #[test]
    fn an_empty_registry_says_so_rather_than_listing_nothing() {
        let config = load(indoc::indoc! {r#"
            [[packages]]
            name = "verctl"
            path = "Cargo.toml"
        "#});
        let err = one(&config, "ci job", "verify", "linux").unwrap_err();
        assert!(format!("{err:#}").contains("none declared"), "{err:#}");
    }

    #[test]
    fn an_empty_runners_list_is_not_a_default() {
        let err = of(
            &load(REGISTRY),
            "ci job",
            "verify",
            Some(&Vec::new()),
            "hosted",
            &["ubuntu-latest"],
        )
        .unwrap_err();
        assert!(
            format!("{err:#}").contains("at least one machine"),
            "{err:#}"
        );
    }

    #[test]
    fn naming_one_machine_twice_is_rejected() {
        let err = of(
            &load(REGISTRY),
            "ci job",
            "verify",
            Some(&named(&["linux", "linux"])),
            "hosted",
            &[],
        )
        .unwrap_err();
        assert!(format!("{err:#}").contains("named twice"), "{err:#}");
    }

    #[test]
    fn a_machine_without_labels_is_rejected_even_when_unused() {
        let config = load(indoc::indoc! {r#"
            [[packages]]
            name = "verctl"
            path = "Cargo.toml"
            [runners.broken]
            labels = []
        "#});
        let err = declared(&config).unwrap_err();
        assert!(format!("{err:#}").contains("at least one label"), "{err:#}");
    }

    #[test]
    fn a_blank_label_is_rejected() {
        let config = load(indoc::indoc! {r#"
            [[packages]]
            name = "verctl"
            path = "Cargo.toml"
            [runners.broken]
            labels = ["  "]
        "#});
        let err = declared(&config).unwrap_err();
        assert!(format!("{err:#}").contains("empty label"), "{err:#}");
    }

    #[test]
    fn a_repeated_label_inside_one_machine_is_rejected() {
        let config = load(indoc::indoc! {r#"
            [[packages]]
            name = "verctl"
            path = "Cargo.toml"
            [runners.broken]
            labels = ["linux", "linux"]
        "#});
        let err = declared(&config).unwrap_err();
        assert!(format!("{err:#}").contains("repeated"), "{err:#}");
    }

    #[test]
    fn a_bare_label_list_is_not_a_machine() {
        let err = toml::from_str::<Config>(indoc::indoc! {r#"
            [[packages]]
            name = "verctl"
            path = "Cargo.toml"
            [runners]
            linux = ["ubuntu-latest"]
        "#})
        .unwrap_err();
        assert!(format!("{err}").contains("invalid type"), "{err}");
    }

    #[test]
    fn only_a_fanned_out_job_carries_its_machine_in_the_name() {
        assert_eq!(check_name("verify", "linux", false), "verify");
        assert_eq!(check_name("verify", "linux", true), "verify (linux)");
    }
}
