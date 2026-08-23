use ctl_core::prelude::{Document, Fields, MessageKind, Notice, NoticeLevel, Present, Table};
use serde::Serialize;

use verctl::{assets, ci, publish, versions};

#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum Report {
    Instructions(InstructionsReport),
    Check(CheckReport),
    VersionCheck(VersionCheckReport),
    Status(StatusReport),
    Prepare(PrepareReport),
    Publish(PublishReport),
    Pin(PinReport),
    Assets(AssetsReport),
    Ci(CiReport),
}

impl Present for Report {
    fn present(&self) -> Document {
        match self {
            Self::Instructions(report) => report.present(),
            Self::Check(report) => report.present(),
            Self::VersionCheck(report) => report.present(),
            Self::Status(report) => report.present(),
            Self::Prepare(report) => report.present(),
            Self::Publish(report) => report.present(),
            Self::Pin(report) => report.present(),
            Self::Assets(report) => report.present(),
            Self::Ci(report) => report.present(),
        }
    }

    fn message_kind(&self) -> MessageKind {
        match self {
            Self::VersionCheck(report) => report.message_kind(),
            _ => MessageKind::Success,
        }
    }
}

#[derive(Serialize)]
pub(super) struct InstructionsReport {
    instructions: String,
}

impl InstructionsReport {
    pub(super) fn new(instructions: &str) -> Self {
        Self {
            instructions: instructions.trim_end().to_owned(),
        }
    }
}

impl Present for InstructionsReport {
    fn present(&self) -> Document {
        Document::new().paragraph(self.instructions.clone())
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
            Table::new(["name", "default", "local"]).token_column(0),
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

    fn message_kind(&self) -> MessageKind {
        if self.drifted().is_empty() {
            MessageKind::Success
        } else {
            MessageKind::Error
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
            Table::new(["file", "package", "bump"]).token_column(1),
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
pub(super) struct PrepareReport {
    pub(super) bumps: Vec<PrepareBump>,
    pub(super) changelog: String,
    pub(super) consume: Vec<String>,
    pub(super) pins: Vec<String>,
    pub(super) pr: Option<String>,
    pub(super) next: Vec<String>,
    pub(super) dry_run: bool,
}

#[derive(Clone, Serialize)]
pub(super) struct PrepareBump {
    pub(super) name: String,
    pub(super) from: String,
    pub(super) to: String,
    pub(super) bump: String,
}

impl Present for PrepareReport {
    fn present(&self) -> Document {
        let no_version_change = self.bumps.is_empty()
            && self.changelog.trim().is_empty()
            && self.consume.is_empty()
            && self.pins.is_empty()
            && self.next.is_empty()
            && self.pr.as_deref().is_none_or(|pr| pr == "no-op");
        if no_version_change {
            return Document::new()
                .fields(Fields::new().row("no-op", "no version-changing fragments"));
        }
        let mut document = Document::new();
        if !self.bumps.is_empty() {
            let table = self.bumps.iter().fold(
                Table::new(["name", "from", "to", "bump"]).token_column(0),
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
        for command in &self.next {
            fields = fields.row("next", command.clone());
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
    pub(super) packages: Vec<publish::PublishLine>,
    pub(super) releases: Vec<String>,
    pub(super) dry_run: bool,
}

impl Present for PublishReport {
    fn present(&self) -> Document {
        if self.packages.is_empty() && self.releases.is_empty() {
            return Document::new().fields(Fields::new().row("no-op", "nothing to publish"));
        }
        let with_notes = self.packages.iter().any(|entry| entry.note.is_some());
        let headers = if with_notes {
            vec!["name", "version", "via", "note"]
        } else {
            vec!["name", "version", "via"]
        };
        let table =
            self.packages
                .iter()
                .fold(Table::new(headers).token_column(0), |table, entry| {
                    let mut row =
                        vec![entry.name.clone(), entry.version.clone(), entry.via.clone()];
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
        let mut document = Document::new();
        if !self.packages.is_empty() {
            document = document.table(table);
        }
        if fields.is_empty() {
            document
        } else {
            document.fields(fields)
        }
    }
}

#[derive(Serialize)]
pub(super) struct PinReport {
    pub(super) files: Vec<String>,
}

impl Present for PinReport {
    fn present(&self) -> Document {
        if self.files.is_empty() {
            return Document::new().fields(Fields::new().row("pins", "none"));
        }
        let fields = self.files.iter().fold(Fields::new(), |fields, file| {
            fields.row("pin", file.clone())
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
    use ctl_core::prelude::{ColorMode, OutputFormat, Stream, View};
    use indoc::formatdoc;

    use super::{
        CheckReport, PrepareReport, PublishReport, Report, StatusFragment, StatusPackage,
        StatusReport,
    };

    #[test]
    fn check_report_keeps_pretty_and_json_shapes() {
        let report = Report::Check(CheckReport { ok: 2 });
        let pretty = View::new(OutputFormat::Pretty, ColorMode::Never)
            .width(80)
            .capture(&report)
            .expect("pretty report");
        assert_eq!(pretty.stream(), Stream::Stdout);
        assert_eq!(
            pretty.text(),
            formatdoc! {"
                ┌────┬───────────────┐
                │ ok ┆ 2 fragment(s) │
                └────┴───────────────┘
            "},
        );

        let json = View::new(OutputFormat::Json, ColorMode::Always)
            .capture(&report)
            .expect("JSON report");
        assert_eq!(json.stream(), Stream::Stdout);
        assert_eq!(json.text(), "{\"ok\":2}\n");
        assert!(!json.text().contains('\u{1b}'));
    }

    #[test]
    fn empty_prepare_reports_a_no_op() {
        let report = Report::Prepare(PrepareReport {
            bumps: Vec::new(),
            changelog: String::new(),
            consume: Vec::new(),
            pins: Vec::new(),
            pr: None,
            next: Vec::new(),
            dry_run: false,
        });
        let pretty = View::new(OutputFormat::Pretty, ColorMode::Never)
            .width(80)
            .capture(&report)
            .expect("prepare report");
        assert!(pretty.text().contains("no version-changing fragments"));
    }

    #[test]
    fn releases_without_packages_have_no_empty_table() {
        let report = Report::Publish(PublishReport {
            packages: Vec::new(),
            releases: vec!["https://example.test/v1".into()],
            dry_run: false,
        });
        let pretty = View::new(OutputFormat::Pretty, ColorMode::Never)
            .width(80)
            .capture(&report)
            .expect("publish report");
        assert_eq!(
            pretty.text(),
            formatdoc! {"
                ┌─────────┬─────────────────────────┐
                │ release ┆ https://example.test/v1 │
                └─────────┴─────────────────────────┘
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

                ┌─────────┬───────┐
                │ pending ┆ 1     │
                │ max     ┆ minor │
                └─────────┴───────┘
            "},
        );
        let json = View::new(OutputFormat::Json, ColorMode::Never)
            .capture(&report)
            .expect("JSON report");
        assert_eq!(
            json.text(),
            "{\"pending\":1,\"max\":\"minor\",\"fragments\":[{\"file\":\"change.md\",\"max\":\"minor\",\"packages\":[{\"name\":\"demo\",\"bump\":\"minor\"}]}]}\n",
        );
    }
}
