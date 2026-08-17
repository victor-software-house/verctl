//! Which machines run PR and push validation.
//!
//! `[[ci.jobs]]` is the same shape as `[[assets.targets]]`: `id` names the
//! job, `runs_on` is the label list GitHub receives. Write nothing and one
//! `verify` job runs on `ubuntu-latest`, which is what every repo in this
//! family did before the table existed.
//!
//! GitHub needs `runs-on` before a job exists, so a job cannot read its own
//! value from verctl. A small `plan` job emits this matrix and the real jobs
//! consume it — the same bootstrap the publish lane already uses for assets.

use crate::config::Config;
use crate::runners;
use anyhow::{Context, Result};
use serde::Serialize;
use std::fs;
use std::path::Path;

const DEFAULT_JOB: &str = "verify";
const DEFAULT_RUNS_ON: &[&str] = &["ubuntu-latest"];

/// One row of a GitHub Actions `strategy.matrix`, and one job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CiJob {
    pub id: String,
    pub runs_on: Vec<String>,
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
    if config.ci.jobs.is_empty() {
        return Ok(CiPlan {
            matrix: Matrix {
                include: vec![CiJob {
                    id: DEFAULT_JOB.to_owned(),
                    runs_on: runners::labels("ci job", DEFAULT_JOB, None, DEFAULT_RUNS_ON)?,
                }],
            },
        });
    }
    runners::unique("ci job", config.ci.jobs.iter().map(|job| job.id.as_str()))?;
    let mut include = Vec::new();
    for job in &config.ci.jobs {
        include.push(CiJob {
            id: job.id.clone(),
            runs_on: runners::labels("ci job", &job.id, job.runs_on.as_ref(), DEFAULT_RUNS_ON)?,
        });
    }
    Ok(CiPlan {
        matrix: Matrix { include },
    })
}

pub fn write_github_output(plan: &CiPlan, path: &Path) -> Result<()> {
    let encoded = serde_json::to_string(&plan.matrix).context("encode matrix")?;
    fs::write(path, format!("matrix={encoded}\n")).with_context(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::plan;
    use crate::config::Config;

    fn load(toml: &str) -> Config {
        toml::from_str(toml).expect("parse")
    }

    const PACKAGE: &str = indoc::indoc! {r#"
        [[packages]]
        name = "verctl"
        path = "Cargo.toml"
    "#};

    #[test]
    fn no_ci_table_is_one_verify_on_ubuntu_latest() {
        let planned = plan(&load(PACKAGE)).unwrap();
        assert_eq!(planned.matrix.include.len(), 1);
        assert_eq!(planned.matrix.include[0].id, "verify");
        assert_eq!(planned.matrix.include[0].runs_on, ["ubuntu-latest"]);
    }

    #[test]
    fn an_id_alone_still_takes_the_default_runner() {
        let planned = plan(&load(indoc::indoc! {r#"
            [[packages]]
            name = "verctl"
            path = "Cargo.toml"
            [[ci.jobs]]
            id = "verify"
        "#}))
        .unwrap();
        assert_eq!(planned.matrix.include[0].runs_on, ["ubuntu-latest"]);
    }

    #[test]
    fn a_label_list_is_passed_through() {
        let planned = plan(&load(indoc::indoc! {r#"
            [[packages]]
            name = "verctl"
            path = "Cargo.toml"
            [[ci.jobs]]
            id = "verify"
            runs_on = ["self-hosted", "linux", "x64"]
        "#}))
        .unwrap();
        assert_eq!(
            planned.matrix.include[0].runs_on,
            ["self-hosted", "linux", "x64"]
        );
    }

    #[test]
    fn the_list_holds_more_than_one_job() {
        let planned = plan(&load(indoc::indoc! {r#"
            [[packages]]
            name = "verctl"
            path = "Cargo.toml"
            [[ci.jobs]]
            id = "verify"
            [[ci.jobs]]
            id = "verify-macos"
            runs_on = ["macos-latest"]
        "#}))
        .unwrap();
        assert_eq!(
            planned
                .matrix
                .include
                .iter()
                .map(|job| job.id.as_str())
                .collect::<Vec<_>>(),
            ["verify", "verify-macos"]
        );
    }

    #[test]
    fn duplicate_job_ids_are_rejected() {
        let err = plan(&load(indoc::indoc! {r#"
            [[packages]]
            name = "verctl"
            path = "Cargo.toml"
            [[ci.jobs]]
            id = "verify"
            [[ci.jobs]]
            id = "verify"
            runs_on = ["macos-latest"]
        "#}))
        .unwrap_err();
        assert!(format!("{err:#}").contains("duplicate ci job"), "{err:#}");
    }

    #[test]
    fn an_empty_label_list_is_rejected() {
        let err = plan(&load(indoc::indoc! {r#"
            [[packages]]
            name = "verctl"
            path = "Cargo.toml"
            [[ci.jobs]]
            id = "verify"
            runs_on = []
        "#}))
        .unwrap_err();
        assert!(format!("{err:#}").contains("at least one label"), "{err:#}");
    }

    #[test]
    fn a_build_field_is_not_a_ci_field() {
        let err = toml::from_str::<Config>(indoc::indoc! {r#"
            [[packages]]
            name = "verctl"
            path = "Cargo.toml"
            [[ci.jobs]]
            id = "verify"
            triple = "x86_64-unknown-linux-gnu"
        "#})
        .unwrap_err();
        assert!(format!("{err}").contains("triple"), "{err}");
    }

    #[test]
    fn github_output_is_one_matrix_assignment() {
        let planned = plan(&load(PACKAGE)).unwrap();
        let root = tempfile::TempDir::new().unwrap();
        let out = root.path().join("out");
        super::write_github_output(&planned, &out).unwrap();
        let text = std::fs::read_to_string(&out).unwrap();
        assert_eq!(
            text,
            "matrix={\"include\":[{\"id\":\"verify\",\"runs_on\":[\"ubuntu-latest\"]}]}\n"
        );
    }
}
