//! Which machines run PR and push validation.
//!
//! A `ci` entry is one job; its `runners` name machines from `runners`, one
//! check each. Write nothing and one `verify` job runs on `ubuntu-latest`,
//! which is what every repo in this family did before the section existed.
//!
//! GitHub needs `runs-on` before a job exists, so a job cannot read its own
//! machine from verctl. A small `plan` job emits this matrix and the real jobs
//! consume it — the same bootstrap the publish lane uses for assets.

use crate::config::Config;
use crate::runners;
use anyhow::{Context, Result};
use serde::Serialize;
use std::path::Path;

const DEFAULT_JOB: &str = "verify";
const DEFAULT_MACHINE: &str = "ubuntu-latest";
const DEFAULT_LABELS: &[&str] = &["ubuntu-latest"];

/// One row of a GitHub Actions `strategy.matrix`, and one check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CiJob {
    /// The job as declared. Rows share it when the job fans out.
    pub id: String,
    /// The check name, unique across rows.
    pub name: String,
    /// The machine this row runs on, as declared.
    pub machine: String,
    /// Straight into `runs-on:`.
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Matrix {
    pub include: Vec<CiJob>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CiPlan {
    pub matrix: Matrix,
}

pub fn plan(config: &Config) -> Result<CiPlan> {
    runners::declared(config)?;
    let mut include = Vec::new();
    if config.ci.is_empty() {
        let resolved = machines(config, DEFAULT_JOB, None)?;
        push(&mut include, DEFAULT_JOB, resolved);
        return Ok(CiPlan {
            matrix: Matrix { include },
        });
    }
    for (id, job) in &config.ci {
        let resolved = machines(config, id, job.runners.as_ref())?;
        push(&mut include, id, resolved);
    }
    Ok(CiPlan {
        matrix: Matrix { include },
    })
}

fn machines(
    config: &Config,
    id: &str,
    named: Option<&Vec<String>>,
) -> Result<Vec<runners::Machine>> {
    runners::of(config, "ci job", id, named, DEFAULT_MACHINE, DEFAULT_LABELS)
}

fn push(include: &mut Vec<CiJob>, id: &str, resolved: Vec<runners::Machine>) {
    let fanned_out = resolved.len() > 1;
    for machine in resolved {
        include.push(CiJob {
            id: id.to_owned(),
            name: runners::check_name(id, &machine.name, fanned_out),
            machine: machine.name,
            labels: machine.labels,
        });
    }
}

pub fn write_github_output(plan: &CiPlan, path: &Path) -> Result<()> {
    let encoded = serde_json::to_string(&plan.matrix).context("encode matrix")?;
    crate::github::write_output(path, &format!("matrix={encoded}\n"))
}

#[cfg(test)]
mod tests {
    use super::plan;
    use crate::config::Config;

    fn load(yaml: &str) -> Config {
        Config::parse(yaml).expect("parse")
    }

    const PACKAGE: &str = indoc::indoc! {"
        packages:
          - name: verctl
            path: Cargo.toml
    "};

    const REGISTRY: &str = indoc::indoc! {"
        packages:
          - name: verctl
            path: Cargo.toml

        runners:
          linux:
            labels: [ubuntu-latest]
          macos:
            labels: [macos-latest]
          big:
            labels: [self-hosted, linux, x64]
    "};

    fn with(extra: &str) -> Config {
        load(&format!("{REGISTRY}{extra}"))
    }

    #[test]
    fn no_ci_section_is_one_verify_on_ubuntu_latest() {
        let planned = plan(&load(PACKAGE)).unwrap();
        assert_eq!(planned.matrix.include.len(), 1);
        assert_eq!(planned.matrix.include[0].id, "verify");
        assert_eq!(planned.matrix.include[0].name, "verify");
        assert_eq!(planned.matrix.include[0].labels, ["ubuntu-latest"]);
    }

    #[test]
    fn a_job_with_no_runners_takes_the_default_machine() {
        let planned = plan(&with("ci:\n  verify: {}\n")).unwrap();
        assert_eq!(planned.matrix.include[0].labels, ["ubuntu-latest"]);
        assert_eq!(planned.matrix.include[0].name, "verify");
    }

    #[test]
    fn one_machine_with_three_labels_is_one_check() {
        let planned = plan(&with(indoc::indoc! {"
            ci:
              verify:
                runners: [big]
        "}))
        .unwrap();
        assert_eq!(planned.matrix.include.len(), 1);
        assert_eq!(planned.matrix.include[0].name, "verify");
        assert_eq!(
            planned.matrix.include[0].labels,
            ["self-hosted", "linux", "x64"]
        );
    }

    #[test]
    fn two_machines_fan_one_job_into_two_named_checks() {
        let planned = plan(&with(indoc::indoc! {"
            ci:
              verify:
                runners: [linux, macos]
        "}))
        .unwrap();
        assert_eq!(planned.matrix.include.len(), 2);
        assert!(planned.matrix.include.iter().all(|row| row.id == "verify"));
        assert_eq!(
            planned
                .matrix
                .include
                .iter()
                .map(|row| row.name.as_str())
                .collect::<Vec<_>>(),
            ["verify (linux)", "verify (macos)"]
        );
    }

    #[test]
    fn separate_jobs_stay_separate_checks() {
        let planned = plan(&with(indoc::indoc! {"
            ci:
              audit:
                runners: [linux]
              verify:
                runners: [macos]
        "}))
        .unwrap();
        assert_eq!(
            planned
                .matrix
                .include
                .iter()
                .map(|row| row.name.as_str())
                .collect::<Vec<_>>(),
            ["audit", "verify"]
        );
    }

    #[test]
    fn an_undeclared_machine_is_rejected() {
        let err = plan(&with(indoc::indoc! {"
            ci:
              verify:
                runners: [windows]
        "}))
        .unwrap_err();
        let text = format!("{err:#}");
        assert!(text.contains("not declared in runners"), "{text}");
        assert!(text.contains("big, linux, macos"), "{text}");
    }

    #[test]
    fn an_empty_runners_list_is_rejected() {
        let err = plan(&with(indoc::indoc! {"
            ci:
              verify:
                runners: []
        "}))
        .unwrap_err();
        assert!(
            format!("{err:#}").contains("at least one machine"),
            "{err:#}"
        );
    }

    #[test]
    fn a_build_field_is_not_a_ci_field() {
        let err = Config::parse(&format!(
            "{REGISTRY}{}",
            indoc::indoc! {"
                ci:
                  verify:
                    triple: x86_64-unknown-linux-gnu
            "}
        ))
        .unwrap_err();
        assert!(format!("{err:#}").contains("triple"), "{err:#}");
    }

    #[test]
    fn a_label_is_not_a_machine_name() {
        let err = plan(&with(indoc::indoc! {"
            ci:
              verify:
                runners: [ubuntu-latest]
        "}))
        .unwrap_err();
        assert!(
            format!("{err:#}").contains("not declared in runners"),
            "{err:#}"
        );
    }

    #[test]
    fn github_output_is_one_matrix_assignment() {
        let planned = plan(&load(PACKAGE)).unwrap();
        let root = tempfile::TempDir::new().unwrap();
        let out = root.path().join("out");
        super::write_github_output(&planned, &out).unwrap();
        let text = std::fs::read_to_string(&out).unwrap();
        let matrix_line = text
            .lines()
            .find(|line| line.starts_with("matrix="))
            .unwrap();
        let json: serde_json::Value =
            serde_json::from_str(matrix_line.trim_start_matches("matrix=")).unwrap();
        let first = &json["include"][0];
        assert_eq!(first["id"], "verify");
        assert_eq!(first["name"], "verify");
        assert_eq!(first["labels"][0], "ubuntu-latest");
        assert_eq!(text.lines().count(), 1, "{text}");
    }

    #[test]
    fn github_output_keeps_what_an_earlier_step_wrote() {
        let planned = plan(&load(PACKAGE)).unwrap();
        let root = tempfile::TempDir::new().unwrap();
        let out = root.path().join("out");
        std::fs::write(&out, "earlier=kept\n").unwrap();
        super::write_github_output(&planned, &out).unwrap();
        let text = std::fs::read_to_string(&out).unwrap();
        assert!(text.starts_with("earlier=kept\n"), "{text}");
        assert!(text.contains("matrix={"), "{text}");
    }

    #[test]
    fn github_output_does_not_fuse_onto_an_unterminated_line() {
        let planned = plan(&load(PACKAGE)).unwrap();
        let root = tempfile::TempDir::new().unwrap();
        let out = root.path().join("out");
        std::fs::write(&out, "earlier=kept").unwrap();
        super::write_github_output(&planned, &out).unwrap();
        let text = std::fs::read_to_string(&out).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "earlier=kept");
        assert!(lines[1].starts_with("matrix={"), "{text}");
    }
}
