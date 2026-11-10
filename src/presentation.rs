use ctl_core::prelude::{Document, Fields, Notice, NoticeLevel, Outcome, Present, Table};
use serde::Serialize;
use verctl::{assets, ci, docsrs, publish, semver_check, versions};

#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum Report {
    Instructions(InstructionsReport),
    Check(CheckReport),
    VersionCheck(VersionCheckReport),
    Status(StatusReport),
    Version(VersionReport),
    Publish(PublishReport),
    Render(RenderReport),
    Assets(AssetsReport),
    Vendored(VendoredReport),
    Ci(CiReport),
    Docs(DocsReport),
    Semver(SemverReport),
}

impl Present for Report {
    fn present(&self) -> Document {
        match self {
            Self::Instructions(report) => report.present(),
            Self::Check(report) => report.present(),
            Self::VersionCheck(report) => report.present(),
            Self::Status(report) => report.present(),
            Self::Version(report) => report.present(),
            Self::Publish(report) => report.present(),
            Self::Render(report) => report.present(),
            Self::Assets(report) => report.present(),
            Self::Vendored(report) => report.present(),
            Self::Ci(report) => report.present(),
            Self::Docs(report) => report.present(),
            Self::Semver(report) => report.present(),
        }
    }

    fn outcome(&self) -> Outcome {
        match self {
            Self::VersionCheck(report) => report.outcome(),
            Self::Instructions(_)
            | Self::Check(_)
            | Self::Status(_)
            | Self::Version(_)
            | Self::Publish(_)
            | Self::Render(_)
            | Self::Assets(_)
            | Self::Vendored(_)
            | Self::Ci(_)
            | Self::Docs(_)
            | Self::Semver(_) => Outcome::Success,
        }
    }
}

#[derive(Serialize)]
pub(super) struct DocsReport {
    pub(super) crates: Vec<docsrs::DocsLine>,
}

impl Present for DocsReport {
    fn present(&self) -> Document {
        if self.crates.is_empty() {
            return Document::new().fields(Fields::new().row("no-op", "no crate to check"));
        }
        let table = self.crates.iter().fold(
            Table::new(vec!["name", "version", "docs.rs"]).id_column(0),
            |table, line| {
                let status = match line.status {
                    docsrs::DocsStatus::Built => "built",
                    docsrs::DocsStatus::Failed => "failed",
                    docsrs::DocsStatus::Unbuilt => "unbuilt",
                };
                table.row(vec![
                    line.name.clone(),
                    line.version.clone(),
                    status.to_owned(),
                ])
            },
        );
        Document::new().table(table)
    }
}

#[derive(Serialize)]
pub(super) struct SemverReport {
    pub(super) crates: Vec<semver_check::SemverLine>,
}

impl Present for SemverReport {
    fn present(&self) -> Document {
        if self.crates.is_empty() {
            return Document::new().fields(Fields::new().row("no-op", "no crates.io crate"));
        }
        let table = self.crates.iter().fold(
            Table::new(vec!["name", "version", "baseline", "semver"]).id_column(0),
            |table, line| {
                let decision = match line.decision {
                    semver_check::Decision::Passed => "passed",
                    semver_check::Decision::Failed => "failed",
                    semver_check::Decision::MinorPending => "skipped: minor fragment",
                    semver_check::Decision::VersionMoved => "skipped: minor moved",
                    semver_check::Decision::Unpublished => "skipped: unpublished",
                };
                table.row(vec![
                    line.name.clone(),
                    line.version.clone(),
                    line.baseline.clone().unwrap_or_else(|| "-".into()),
                    decision.to_owned(),
                ])
            },
        );
        Document::new().table(table)
    }
}

#[derive(Serialize)]
pub(super) struct InstructionsReport {
    instructions: String,
}

impl InstructionsReport {
    pub(super) fn new(instructions: &str) -> Self {
        Self {
            instructions: instructions.to_owned(),
        }
    }
}

impl Present for InstructionsReport {
    fn present(&self) -> Document {
        Document::new().verbatim(self.instructions.clone())
    }
}

#[derive(Serialize)]
pub(super) struct CheckReport {
    pub(super) ok: usize,
}

impl Present for CheckReport {
    fn present(&self) -> Document {
        Document::new()
            .fields(Fields::new().row("ok", format!("{count} fragment(s)", count = self.ok)))
    }
}

#[derive(Serialize)]
pub(super) struct VersionCheckReport {
    pub(super) skip: Option<String>,
    pub(super) rows: Vec<versions::VersionRow>,
}

impl VersionCheckReport {
    fn drifted(&self) -> Vec<&versions::VersionRow> {
        self.rows.iter().filter(|row| row.drifted()).collect()
    }

    fn failure(&self) -> Option<String> {
        let report = versions::VersionReport {
            skip: self.skip.clone(),
            rows: self.rows.clone(),
        };
        report.require_clean().err().map(|error| error.to_string())
    }
}

impl Present for VersionCheckReport {
    fn present(&self) -> Document {
        if let Some(skip) = &self.skip {
            return Document::new().fields(Fields::new().row("exempt", skip.clone()));
        }
        let drifted = self.drifted();
        if drifted.is_empty() {
            return Document::new().fields(Fields::new().row("versions", "match"));
        }
        let table = drifted.iter().fold(
            Table::new(["name", "default", "local"]).id_column(0),
            |table, row| {
                table.row([
                    row.name.clone(),
                    row.remote.clone().unwrap_or_else(|| "-".into()),
                    row.local.clone(),
                ])
            },
        );
        let document = Document::new().table(table);
        if let Some(message) = self.failure() {
            document.notice(Notice::new(NoticeLevel::Error, message))
        } else {
            document
        }
    }

    fn outcome(&self) -> Outcome {
        if self.drifted().is_empty() {
            Outcome::Success
        } else {
            Outcome::Error
        }
    }
}

#[derive(Serialize)]
pub(super) struct StatusReport {
    pub(super) pending: usize,
    pub(super) max: String,
    pub(super) fragments: Vec<StatusFragment>,
}

#[derive(Serialize)]
pub(super) struct StatusFragment {
    pub(super) file: String,
    pub(super) max: String,
    pub(super) packages: Vec<StatusPackage>,
}

#[derive(Serialize)]
pub(super) struct StatusPackage {
    pub(super) name: String,
    pub(super) bump: String,
}

impl Present for StatusReport {
    fn present(&self) -> Document {
        if self.pending == 0 {
            return Document::new().fields(Fields::new().row("pending", "0"));
        }
        let table = self.fragments.iter().fold(
            Table::new(["file", "package", "bump"]).id_column(1),
            |table, fragment| {
                fragment.packages.iter().fold(table, |table, package| {
                    table.row([
                        fragment.file.clone(),
                        package.name.clone(),
                        package.bump.clone(),
                    ])
                })
            },
        );
        Document::new().table(table).fields(
            Fields::new()
                .row("pending", self.pending.to_string())
                .row("max", self.max.clone()),
        )
    }
}

#[derive(Clone, Serialize)]
pub(super) struct VersionReport {
    pub(super) bumps: Vec<VersionBump>,
    pub(super) changelog: String,
    pub(super) consume: Vec<String>,
    pub(super) pins: Vec<String>,
    pub(super) pr: Option<String>,
    pub(super) after: Vec<String>,
    pub(super) dry_run: bool,
}

#[derive(Clone, Serialize)]
pub(super) struct VersionBump {
    pub(super) name: String,
    pub(super) from: String,
    pub(super) to: String,
    pub(super) bump: String,
}

impl Present for VersionReport {
    fn present(&self) -> Document {
        let no_version_change = self.bumps.is_empty()
            && self.changelog.trim().is_empty()
            && self.consume.is_empty()
            && self.pins.is_empty()
            && self.after.is_empty()
            && self.pr.as_deref().is_none_or(|pr| pr == "no-op");
        if no_version_change {
            let mut fields = Fields::new().row("no-op", "no version-changing fragments");
            if self.dry_run {
                fields = fields.row("dry-run", "nothing written");
            }
            return Document::new().fields(fields);
        }
        let mut document = Document::new();
        if !self.bumps.is_empty() {
            let table = self.bumps.iter().fold(
                Table::new(["name", "from", "to", "bump"]).id_column(0),
                |table, bump| {
                    table.row([
                        bump.name.clone(),
                        bump.from.clone(),
                        bump.to.clone(),
                        bump.bump.clone(),
                    ])
                },
            );
            document = document.table(table);
        }
        if !self.changelog.trim().is_empty() {
            document = document.paragraph(self.changelog.trim_end().to_owned());
        }
        let mut fields = Fields::new();
        for file in &self.consume {
            fields = fields.row("consume", file.clone());
        }
        for file in &self.pins {
            fields = fields.row("pin", file.clone());
        }
        if let Some(pr) = &self.pr {
            fields = fields.row("pr", pr.clone());
        }
        for command in &self.after {
            fields = fields.row("after", command.clone());
        }
        if self.dry_run {
            fields = fields.row("dry-run", "nothing written");
        }
        if fields.is_empty() {
            document
        } else {
            document.fields(fields)
        }
    }
}

#[derive(Serialize)]
pub(super) struct PublishReport {
    pub(super) outcome: publish::Outcome,
    pub(super) packages: Vec<publish::PublishLine>,
    pub(super) releases: Vec<String>,
    /// Incomplete packages left for the run at their own version commit.
    pub(super) waiting: Vec<ReleaseRow>,
    /// `--plan`: every incomplete package, at its version commit.
    pub(super) pending: Vec<ReleaseRow>,
    /// The proofs the operator allowed this run to go past.
    pub(super) allowed: Vec<publish::Allowed>,
    pub(super) dry_run: bool,
}

impl PublishReport {
    /// One `allowed` row per proof the run went past, so the log of a run
    /// that went past one says so.
    fn allowed_rows(&self, fields: Fields) -> Fields {
        self.allowed.iter().fold(fields, |fields, allowed| {
            fields.row(
                "allowed",
                format!("{}: {}", allowed.check.name(), allowed.reason),
            )
        })
    }
}

/// A package at the version commit that releases it.
#[derive(Serialize)]
pub(super) struct ReleaseRow {
    pub(super) name: String,
    pub(super) version: String,
    pub(super) commit: String,
}

impl From<&publish::Waiting> for ReleaseRow {
    fn from(waiting: &publish::Waiting) -> Self {
        Self {
            name: waiting.name.clone(),
            version: waiting.version.clone(),
            commit: waiting.commit.clone(),
        }
    }
}

fn release_rows(rows: &[ReleaseRow]) -> Table {
    rows.iter().fold(
        Table::new(vec!["name", "version", "commit"]).id_column(0),
        |table, row| {
            table.row(vec![
                row.name.clone(),
                row.version.clone(),
                row.commit.clone(),
            ])
        },
    )
}

impl Present for PublishReport {
    fn present(&self) -> Document {
        match self.outcome {
            publish::Outcome::NothingToRelease => {
                return Document::new()
                    .fields(self.allowed_rows(Fields::new().row("outcome", "nothing to release")));
            }
            publish::Outcome::Released => {}
        }
        if !self.pending.is_empty() {
            return Document::new().table(release_rows(&self.pending));
        }
        if self.packages.is_empty() && self.releases.is_empty() {
            return Document::new().fields(Fields::new().row("no-op", "nothing to publish"));
        }
        let with_notes = self.packages.iter().any(|entry| entry.note.is_some());
        let headers = if with_notes {
            vec!["name", "version", "via", "note"]
        } else {
            vec!["name", "version", "via"]
        };
        let table = self
            .packages
            .iter()
            .fold(Table::new(headers).id_column(0), |table, entry| {
                let mut row = vec![entry.name.clone(), entry.version.clone(), entry.via.clone()];
                if with_notes {
                    row.push(entry.note.clone().unwrap_or_default());
                }
                table.row(row)
            });
        let mut fields = Fields::new();
        for release in &self.releases {
            fields = fields.row("release", release.clone());
        }
        if self.dry_run {
            fields = fields.row("dry-run", "nothing published");
        }
        let fields = self.allowed_rows(fields);
        let mut document = Document::new();
        if !self.packages.is_empty() {
            document = document.table(table);
        }
        if !self.waiting.is_empty() {
            document = document.table(release_rows(&self.waiting));
        }
        if fields.is_empty() {
            document
        } else {
            document.fields(fields)
        }
    }
}

#[derive(Serialize)]
pub(super) struct RenderReport {
    pub(super) files: Vec<String>,
}

impl Present for RenderReport {
    fn present(&self) -> Document {
        if self.files.is_empty() {
            return Document::new().fields(Fields::new().row("files", "none"));
        }
        let fields = self.files.iter().fold(Fields::new(), |fields, file| {
            fields.row("file", file.clone())
        });
        Document::new().fields(fields)
    }
}

#[derive(Serialize)]
pub(super) struct AssetsReport {
    #[serde(flatten)]
    pub(super) plan: assets::AssetsPlan,
    pub(super) tarball: Option<String>,
    pub(super) uploaded: Option<String>,
}

impl Present for AssetsReport {
    fn present(&self) -> Document {
        if !self.plan.has_assets {
            return Document::new()
                .fields(Fields::new().row("assets", "none (library or one host build is enough)"));
        }
        let table = self.plan.matrix.include.iter().fold(
            Table::new(["target", "runs-on", "asset"]).token_column(0),
            |table, row| table.row([row.id.clone(), row.labels.join(", "), row.asset.clone()]),
        );
        let mut fields = Fields::new().row("tag", self.plan.tag.clone());
        if let Some(path) = &self.tarball {
            fields = fields.row("tarball", path.clone());
        }
        if let Some(url) = &self.uploaded {
            fields = fields.row("upload", url.clone());
        }
        Document::new().table(table).fields(fields)
    }
}

#[derive(Serialize)]
pub(super) struct VendoredReport {
    #[serde(flatten)]
    pub(super) plan: assets::VendoredPlan,
    pub(super) archive: Option<String>,
    pub(super) uploaded: Option<assets::VendoredUpload>,
}

impl Present for VendoredReport {
    fn present(&self) -> Document {
        let table = self.plan.targets.iter().fold(
            Table::new(["target", "runs-on", "asset"]).token_column(0),
            |table, target| {
                table.row([
                    target.id.clone(),
                    target.labels.join(", "),
                    target.asset.clone(),
                ])
            },
        );
        let mut fields = Fields::new()
            .row("vendored", self.plan.name.clone())
            .row("version", self.plan.version.clone())
            .row("tag", self.plan.tag.clone());
        if let Some(path) = &self.archive {
            fields = fields.row("archive", path.clone());
        }
        if let Some(uploaded) = &self.uploaded {
            fields = fields
                .row("release", uploaded.release.clone())
                .row("upload", uploaded.archive.clone())
                .row("checksum", uploaded.checksum.clone());
        }
        Document::new().table(table).fields(fields)
    }
}

#[derive(Serialize)]
pub(super) struct CiReport {
    #[serde(flatten)]
    pub(super) plan: ci::CiPlan,
}

impl Present for CiReport {
    fn present(&self) -> Document {
        let table = self.plan.matrix.include.iter().fold(
            Table::new(["check", "runs-on"]).token_column(0),
            |table, job| table.row([job.name.clone(), job.labels.join(", ")]),
        );
        Document::new().table(table)
    }
}

#[cfg(test)]
mod tests {
    use ctl_core::prelude::{ColorMode, Destination, OutputFormat, View};
    use indoc::formatdoc;
    use verctl::publish;

    use super::{
        CheckReport, PublishReport, ReleaseRow, Report, StatusFragment, StatusPackage,
        StatusReport, VersionReport,
    };

    fn publish_report(outcome: publish::Outcome, pending: Vec<ReleaseRow>) -> Report {
        Report::Publish(PublishReport {
            outcome,
            packages: Vec::new(),
            releases: Vec::new(),
            waiting: Vec::new(),
            pending,
            allowed: Vec::new(),
            dry_run: false,
        })
    }

    /// A run that went past a proof says so on its own row, and the JSON
    /// lists it under `allowed`.
    #[test]
    fn an_allowed_proof_is_on_the_record_in_both_shapes() {
        let report = Report::Publish(PublishReport {
            outcome: publish::Outcome::NothingToRelease,
            packages: Vec::new(),
            releases: Vec::new(),
            waiting: Vec::new(),
            pending: Vec::new(),
            allowed: vec![publish::Allowed {
                check: publish::Check::WorkflowChange,
                reason: ".github/workflows/ at HEAD aaa differs from the default branch at bbb"
                    .into(),
            }],
            dry_run: false,
        });
        let pretty = View::new(OutputFormat::Pretty, ColorMode::Never)
            .width(120)
            .capture(&report)
            .expect("pretty");
        assert_eq!(
            pretty.text(),
            formatdoc! {"
                outcome  nothing to release
                allowed  workflow-change: .github/workflows/ at HEAD aaa differs from the default branch at bbb
            "},
        );
        let json = View::new(OutputFormat::Json, ColorMode::Never)
            .width(80)
            .capture(&report)
            .expect("json");
        let value: serde_json::Value = serde_json::from_str(json.text()).expect("JSON");
        assert_eq!(value["allowed"][0]["check"], "workflow-change");
        assert_eq!(
            value["allowed"][0]["reason"],
            ".github/workflows/ at HEAD aaa differs from the default branch at bbb"
        );
    }

    #[test]
    fn nothing_to_release_says_so_in_both_shapes() {
        let report = publish_report(publish::Outcome::NothingToRelease, Vec::new());
        let pretty = View::new(OutputFormat::Pretty, ColorMode::Never)
            .width(80)
            .capture(&report)
            .expect("pretty");
        assert_eq!(
            pretty.text(),
            formatdoc! {"
                outcome  nothing to release
            "},
        );
        let json = View::new(OutputFormat::Json, ColorMode::Never)
            .width(80)
            .capture(&report)
            .expect("json");
        let value: serde_json::Value = serde_json::from_str(json.text()).expect("JSON");
        assert_eq!(value["outcome"], "nothing-to-release");
    }

    #[test]
    fn a_plan_lists_each_package_at_its_version_commit() {
        let report = publish_report(
            publish::Outcome::Released,
            vec![ReleaseRow {
                name: "core".into(),
                version: "0.3.1".into(),
                commit: "aaa".into(),
            }],
        );
        let pretty = View::new(OutputFormat::Pretty, ColorMode::Never)
            .width(80)
            .capture(&report)
            .expect("pretty");
        assert!(pretty.text().contains("core"), "{}", pretty.text());
        assert!(pretty.text().contains("aaa"), "{}", pretty.text());
    }

    #[test]
    fn check_report_keeps_pretty_and_json_shapes() {
        let report = Report::Check(CheckReport { ok: 2 });
        let pretty = View::new(OutputFormat::Pretty, ColorMode::Never)
            .width(80)
            .capture(&report)
            .expect("pretty report");
        assert_eq!(pretty.stream(), Destination::Stdout);
        assert_eq!(
            pretty.text(),
            formatdoc! {"
                ok  2 fragment(s)
            "},
        );

        let json = View::new(OutputFormat::Json, ColorMode::Always)
            .capture(&report)
            .expect("JSON report");
        assert_eq!(json.stream(), Destination::Stdout);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(json.text()).expect("JSON"),
            serde_json::json!({ "ok": 2 })
        );
        assert!(!json.text().contains('\u{1b}'));
    }

    #[test]
    fn empty_version_dry_run_reports_both_states() {
        let report = Report::Version(VersionReport {
            bumps: Vec::new(),
            changelog: String::new(),
            consume: Vec::new(),
            pins: Vec::new(),
            pr: None,
            after: Vec::new(),
            dry_run: true,
        });
        let pretty = View::new(OutputFormat::Pretty, ColorMode::Never)
            .width(80)
            .capture(&report)
            .expect("version report");
        assert!(pretty.text().contains("no version-changing fragments"));
        assert!(pretty.text().contains("nothing written"));
    }

    /// A release URL is copied out of CI logs, so the fallback width must not
    /// break it across lines.
    #[test]
    fn release_urls_stay_whole_at_the_fallback_width() {
        let url = "https://github.com/victor-software-house/example-repository/releases/tag/example-package@12.34.56";
        let report = Report::Publish(PublishReport {
            outcome: publish::Outcome::Released,
            packages: Vec::new(),
            releases: vec![url.into()],
            waiting: Vec::new(),
            pending: Vec::new(),
            allowed: Vec::new(),
            dry_run: false,
        });
        let pretty = View::new(OutputFormat::Pretty, ColorMode::Never)
            .width(80)
            .capture(&report)
            .expect("publish report");
        assert!(
            pretty.text().lines().any(|line| line.contains(url)),
            "{}",
            pretty.text()
        );
    }

    #[test]
    fn releases_without_packages_have_no_empty_table() {
        let report = Report::Publish(PublishReport {
            outcome: publish::Outcome::Released,
            packages: Vec::new(),
            releases: vec!["https://example.test/v1".into()],
            waiting: Vec::new(),
            pending: Vec::new(),
            allowed: Vec::new(),
            dry_run: false,
        });
        let pretty = View::new(OutputFormat::Pretty, ColorMode::Never)
            .width(80)
            .capture(&report)
            .expect("publish report");
        assert_eq!(
            pretty.text(),
            formatdoc! {"
                release  https://example.test/v1
            "},
        );
    }

    #[test]
    fn status_report_uses_one_model_for_table_and_json() {
        let report = Report::Status(StatusReport {
            pending: 1,
            max: "minor".into(),
            fragments: vec![StatusFragment {
                file: "change.md".into(),
                max: "minor".into(),
                packages: vec![StatusPackage {
                    name: "demo".into(),
                    bump: "minor".into(),
                }],
            }],
        });
        let pretty = View::new(OutputFormat::Pretty, ColorMode::Never)
            .width(80)
            .capture(&report)
            .expect("pretty report");
        assert_eq!(
            pretty.text(),
            formatdoc! {"
                ┌───────────┬─────────┬───────┐
                │ file      ┆ package ┆ bump  │
                ╞═══════════╪═════════╪═══════╡
                │ change.md ┆ demo    ┆ minor │
                └───────────┴─────────┴───────┘

                pending  1
                    max  minor
            "},
        );
        let json = View::new(OutputFormat::Json, ColorMode::Never)
            .capture(&report)
            .expect("JSON report");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(json.text()).expect("JSON"),
            serde_json::json!({
                "pending": 1,
                "max": "minor",
                "fragments": [{
                    "file": "change.md",
                    "max": "minor",
                    "packages": [{ "name": "demo", "bump": "minor" }],
                }],
            })
        );
    }
}
