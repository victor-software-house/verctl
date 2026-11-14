use ctl_core::prelude::{Document, Fields, Notice, NoticeLevel, Outcome, Present, Table};
use serde::Serialize;
use verctl::{assets, ci, first_publish, publish, registry_check, semver_check, stop, versions};

#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum Report {
    Instructions(InstructionsReport),
    Check(CheckReport),
    Status(StatusReport),
    Version(VersionReport),
    Release(ReleaseReport),
    Crates(CratesReport),
    Render(RenderReport),
    Assets(AssetsReport),
    Vendored(VendoredReport),
    Ci(CiReport),
    Semver(SemverReport),
}

impl Present for Report {
    fn present(&self) -> Document {
        match self {
            Self::Instructions(report) => report.present(),
            Self::Check(report) => report.present(),
            Self::Status(report) => report.present(),
            Self::Version(report) => report.present(),
            Self::Release(report) => report.present(),
            Self::Crates(report) => report.present(),
            Self::Render(report) => report.present(),
            Self::Assets(report) => report.present(),
            Self::Vendored(report) => report.present(),
            Self::Ci(report) => report.present(),
            Self::Semver(report) => report.present(),
        }
    }

    fn outcome(&self) -> Outcome {
        match self {
            Self::Check(report) => report.outcome(),
            Self::Release(report) => report.outcome(),
            Self::Crates(report) => report.outcome(),
            Self::Instructions(_)
            | Self::Status(_)
            | Self::Version(_)
            | Self::Render(_)
            | Self::Assets(_)
            | Self::Vendored(_)
            | Self::Ci(_)
            | Self::Semver(_) => Outcome::Success,
        }
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
                    semver_check::Decision::BreakingPatch => "skipped: patch! fragment",
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

/// What `check` found: one result per check that ran, in the order
/// `cli::Check` declares them.
#[derive(Serialize)]
pub(super) struct CheckReport {
    pub(super) checks: Vec<CheckResult>,
}

/// The result of one check that finished. A check that cannot finish fails
/// the run with its own error, so it has no result here.
#[derive(Serialize)]
#[serde(tag = "check", rename_all = "kebab-case")]
pub(super) enum CheckResult {
    /// Every pending fragment parses, and `version` would accept it.
    Fragments {
        /// How many fragments are pending.
        ok: usize,
    },
    /// Every served file and pin matches what `render` writes.
    Served,
    /// The manifests' versions against the default branch.
    Versions(VersionCheckReport),
    /// Each crates.io crate's existence and owners, and how its newest
    /// version was published.
    Registry(registry_check::RegistryReport),
    /// Each crates.io crate's trusted publisher entry.
    Trust(registry_check::TrustReport),
}

impl Present for CheckReport {
    /// One row per check, then each check's details in the same order: the
    /// drifted versions, the crates of `registry` and `trust`, and their
    /// notices.
    fn present(&self) -> Document {
        let mut fields = Fields::new();
        for check in &self.checks {
            fields = match check {
                CheckResult::Fragments { ok } => {
                    fields.row("fragments", format!("ok, {ok} fragment(s)"))
                }
                CheckResult::Served => fields.row("served", "ok"),
                CheckResult::Versions(report) => fields.row("versions", report.summary()),
                CheckResult::Registry(report) => fields.row(
                    "registry",
                    crate_summary(report.crates.len(), report.failures().len()),
                ),
                CheckResult::Trust(report) => fields.row("trust", trust_summary(report)),
            };
        }
        self.checks.iter().fold(
            Document::new().fields(fields),
            |document, check| match check {
                CheckResult::Versions(report) => report.details(document),
                CheckResult::Registry(report) => registry_details(report, document),
                CheckResult::Trust(report) => trust_details(report, document),
                CheckResult::Fragments { .. } | CheckResult::Served => document,
            },
        )
    }

    /// An error when any check failed, after every row is shown.
    fn outcome(&self) -> Outcome {
        let failed = self.checks.iter().any(|check| match check {
            CheckResult::Versions(report) => !report.drifted().is_empty(),
            CheckResult::Registry(report) => !report.failures().is_empty(),
            CheckResult::Trust(report) => !report.failures().is_empty(),
            CheckResult::Fragments { .. } | CheckResult::Served => false,
        });
        if failed {
            Outcome::Error
        } else {
            Outcome::Success
        }
    }
}

/// The one-line result of a check over `total` crates, `failed` of them
/// failing.
fn crate_summary(total: usize, failed: usize) -> String {
    match (total, failed) {
        (0, _) => "ok, no crates.io crate".to_owned(),
        (_, 0) => format!("ok, {total} crate(s)"),
        _ => format!("failed, {failed} of {total} crate(s)"),
    }
}

/// The one-line result of `trust`, which fails without a token.
fn trust_summary(report: &registry_check::TrustReport) -> String {
    match report {
        registry_check::TrustReport::Unset { variable } => {
            format!("not checked, {variable} is unset")
        }
        registry_check::TrustReport::Set { crates, .. } => {
            crate_summary(crates.len(), report.failures().len())
        }
    }
}

/// `document` with one warning per notice and one error per failure.
fn with_findings(document: Document, notices: &[String], failures: &[String]) -> Document {
    failures
        .iter()
        .fold(with_warnings(document, notices), |document, failure| {
            document.notice(Notice::new(NoticeLevel::Error, failure.clone()))
        })
}

/// `document` with the `registry` table, its notices, and its failures.
fn registry_details(report: &registry_check::RegistryReport, document: Document) -> Document {
    if report.crates.is_empty() {
        return document;
    }
    let table = report.crates.iter().fold(
        Table::new(["crate", "state", "owners", "trusted"]).id_column(0),
        |table, row| {
            let (state, owners, trusted) = match &row.state {
                registry_check::RegistryState::Missing => {
                    ("missing", "-".to_owned(), "-".to_owned())
                }
                registry_check::RegistryState::New => ("new", "-".to_owned(), "-".to_owned()),
                registry_check::RegistryState::Creates => {
                    ("created by this run", "-".to_owned(), "-".to_owned())
                }
                registry_check::RegistryState::NotAtZero { .. } => {
                    ("not at 0.0.0", "-".to_owned(), "-".to_owned())
                }
                registry_check::RegistryState::Present {
                    owners,
                    lacking,
                    trusted,
                    ..
                } => (
                    if lacking.is_empty() {
                        "ok"
                    } else {
                        "lacks owner"
                    },
                    if owners.is_empty() {
                        "-".to_owned()
                    } else {
                        owners.join(", ")
                    },
                    trusted.label(),
                ),
            };
            table.row([row.name.clone(), state.to_owned(), owners, trusted])
        },
    );
    with_findings(document.table(table), &report.notices(), &report.failures())
}

/// `document` with the `trust` table, its notices, and its failures.
fn trust_details(report: &registry_check::TrustReport, document: Document) -> Document {
    let document = match report {
        registry_check::TrustReport::Set { crates, .. } if !crates.is_empty() => {
            document.table(crates.iter().fold(
                Table::new(["crate", "state", "entries"]).id_column(0),
                |table, row| {
                    let (state, entries) = match &row.state {
                        registry_check::TrustState::Trusted => ("trusted", "-".to_owned()),
                        registry_check::TrustState::Untrusted { entries } => (
                            "no entry",
                            if entries.is_empty() {
                                "-".to_owned()
                            } else {
                                entries
                                    .iter()
                                    .map(|entry| {
                                        format!(
                                            "{owner}/{name} {file}",
                                            owner = entry.repository_owner,
                                            name = entry.repository_name,
                                            file = entry.workflow_filename
                                        )
                                    })
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            },
                        ),
                        registry_check::TrustState::Missing => ("missing", "-".to_owned()),
                        registry_check::TrustState::New => ("new", "-".to_owned()),
                        registry_check::TrustState::Refused { detail } => {
                            ("refused", detail.clone())
                        }
                    };
                    table.row([row.name.clone(), state.to_owned(), entries])
                },
            ))
        }
        registry_check::TrustReport::Set { .. } | registry_check::TrustReport::Unset { .. } => {
            document
        }
    };
    with_findings(document, &report.notices(), &report.failures())
}

#[derive(Serialize)]
pub(super) struct VersionCheckReport {
    pub(super) skip: Option<String>,
    pub(super) rows: Vec<versions::VersionRow>,
    /// The rules skipped because the config at the merge-base does not load.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) notices: Vec<String>,
}

impl VersionCheckReport {
    fn drifted(&self) -> Vec<&versions::VersionRow> {
        self.rows.iter().filter(|row| row.drifted()).collect()
    }

    fn failure(&self) -> Option<String> {
        let report = versions::VersionReport {
            skip: self.skip.clone(),
            rows: self.rows.clone(),
            notices: Vec::new(),
        };
        report.require_clean().err().map(|error| error.to_string())
    }
}

impl VersionCheckReport {
    /// `document` with one warning per skipped rule.
    fn with_notices(&self, document: Document) -> Document {
        self.notices.iter().fold(document, |document, notice| {
            document.notice(Notice::new(NoticeLevel::Warning, notice.clone()))
        })
    }
}

impl VersionCheckReport {
    /// The one-line result: why the comparison was skipped, `match`, or
    /// `differ`.
    fn summary(&self) -> String {
        match &self.skip {
            Some(skip) => format!("exempt, {skip}"),
            None if self.drifted().is_empty() => "match".to_owned(),
            None => "differ".to_owned(),
        }
    }

    /// `document` with the drifted versions, one warning per skipped rule,
    /// and the error that names each drifted package.
    fn details(&self, document: Document) -> Document {
        let drifted = self.drifted();
        if self.skip.is_some() || drifted.is_empty() {
            return self.with_notices(document);
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
        let document = self.with_notices(document.table(table));
        if let Some(message) = self.failure() {
            document.notice(Notice::new(NoticeLevel::Error, message))
        } else {
            document
        }
    }
}

#[derive(Serialize)]
pub(super) struct StatusReport {
    pub(super) pending: usize,
    pub(super) max: String,
    pub(super) fragments: Vec<StatusFragment>,
    /// The asset plan: the packages' when they declare targets, a vendored
    /// set's under `--vendored`, and `null` when there is none.
    pub(super) assets: Option<StatusAssets>,
    /// Every package whose release at HEAD is incomplete. `null` means not
    /// checked, never nothing pending; `unreleased_unchecked` says why.
    pub(super) unreleased: Option<Vec<ReleaseRow>>,
    /// Why `unreleased` was not checked, or `null` when it was.
    pub(super) unreleased_unchecked: Option<Unchecked>,
    /// Every release whose registries list some, but not all, of its
    /// uploads. `null` when `unreleased` was not checked.
    pub(super) partial: Option<Vec<PartialRow>>,
    /// One warning per published, immutable Release that lacks declared
    /// assets.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) notices: Vec<String>,
}

/// A release whose registries list some, but not all, of its uploads, and
/// why the last run stopped, as its draft records it.
#[derive(Serialize)]
pub(super) struct PartialRow {
    pub(super) tag: String,
    /// `null` when a registry lookup failed, so the live uploads were not
    /// counted.
    pub(super) live: Option<usize>,
    pub(super) total: usize,
    /// What the uploads are, plural, such as `crates`.
    pub(super) noun: String,
    /// The draft's stop record: `absent`, `read` with its fields, or
    /// `unreadable`.
    pub(super) stop: stop::Record,
}

impl PartialRow {
    /// One `partial` row, and one `stopped` row when the draft holds a stop
    /// record.
    fn rows(&self, fields: Fields) -> Fields {
        let count = match self.live {
            Some(live) => format!(
                "{live} of {total} {noun} live",
                total = self.total,
                noun = self.noun
            ),
            None => format!(
                "{total} {noun}, live ones not counted (a registry lookup failed)",
                total = self.total,
                noun = self.noun
            ),
        };
        let fields = fields.row(
            "partial",
            format!("{tag}: partially published: {count}", tag = self.tag),
        );
        match &self.stop {
            stop::Record::Absent => fields,
            stop::Record::Read(stopped) => {
                let resume = stopped
                    .retry_after
                    .as_ref()
                    .map_or_else(String::new, |time| format!(", resume after {time}"));
                fields.row(
                    "stopped",
                    format!(
                        "{package} {version}: {cause}{resume} (as the last run reported)",
                        package = stopped.package,
                        version = stopped.version,
                        cause = stopped.cause
                    ),
                )
            }
            stop::Record::Unreadable => fields.row("stopped", "stop record unreadable"),
        }
    }
}

/// `document` with one warning per notice.
fn with_warnings(document: Document, notices: &[String]) -> Document {
    notices.iter().fold(document, |document, notice| {
        document.notice(Notice::new(NoticeLevel::Warning, notice.clone()))
    })
}

/// Why `status` did not check for incomplete releases. The human view
/// shows the top-level message, which fits on one line. JSON holds the whole
/// error chain, so the cause stays available to automation.
#[derive(Clone, Serialize)]
#[serde(into = "String")]
pub(super) struct Unchecked {
    summary: String,
    chain: String,
}

impl Unchecked {
    /// No token was set, so no request reached GitHub.
    pub(super) fn missing_token() -> Self {
        let reason = "no GITHUB_TOKEN or GH_TOKEN".to_owned();
        Self {
            summary: reason.clone(),
            chain: reason,
        }
    }

    /// `--vendored` plans a set, which reads no package, so no release lookup
    /// ran.
    pub(super) fn vendored() -> Self {
        let reason = "a vendored plan reads no packages".to_owned();
        Self {
            summary: reason.clone(),
            chain: reason,
        }
    }

    /// The check ran and failed with `error`.
    pub(super) fn failed(error: &anyhow::Error) -> Self {
        Self {
            summary: error.to_string(),
            chain: format!("{error:#}"),
        }
    }
}

impl From<Unchecked> for String {
    fn from(unchecked: Unchecked) -> Self {
        unchecked.chain
    }
}

/// The asset plan `status` shows.
#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum StatusAssets {
    Packages(assets::AssetsPlan),
    Vendored(assets::VendoredPlan),
}

impl StatusAssets {
    fn table(&self) -> Table {
        let rows: Vec<[String; 3]> = match self {
            Self::Packages(plan) => plan
                .matrix
                .include
                .iter()
                .map(|row| [row.id.clone(), row.labels.join(", "), row.asset.clone()])
                .collect(),
            Self::Vendored(plan) => plan
                .targets
                .iter()
                .map(|target| {
                    [
                        target.id.clone(),
                        target.labels.join(", "),
                        target.asset.clone(),
                    ]
                })
                .collect(),
        };
        rows.into_iter().fold(
            Table::new(["target", "runs-on", "asset"]).token_column(0),
            Table::row,
        )
    }

    fn fields(&self, fields: Fields) -> Fields {
        match self {
            Self::Packages(plan) => fields.row("tag", plan.tag.clone()),
            Self::Vendored(plan) => fields
                .row("vendored", plan.name.clone())
                .row("version", plan.version.clone())
                .row("tag", plan.tag.clone()),
        }
    }
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
        let mut document = Document::new();
        if self.pending > 0 {
            document = document.table(self.fragments.iter().fold(
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
            ));
        }
        if let Some(assets) = &self.assets {
            document = document.table(assets.table());
        }
        if let Some(rows) = &self.unreleased
            && !rows.is_empty()
        {
            document = document.table(release_rows(rows));
        }
        let mut fields = Fields::new().row("pending", self.pending.to_string());
        if self.pending > 0 {
            fields = fields.row("max", self.max.clone());
        }
        fields = match &self.assets {
            Some(assets) => assets.fields(fields),
            None => fields.row("assets", "none"),
        };
        fields = match (&self.unreleased, &self.unreleased_unchecked) {
            (Some(rows), _) => fields.row("unreleased", rows.len().to_string()),
            (None, reason) => fields.row(
                "unreleased",
                format!(
                    "not checked ({})",
                    reason
                        .as_ref()
                        .map_or("unknown", |reason| reason.summary.as_str())
                ),
            ),
        };
        for partial in self.partial.iter().flatten() {
            fields = partial.rows(fields);
        }
        with_warnings(document.fields(fields), &self.notices)
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
pub(super) struct ReleaseReport {
    pub(super) outcome: publish::Outcome,
    pub(super) packages: Vec<publish::PublishLine>,
    pub(super) releases: Vec<String>,
    /// The proofs the operator allowed this run to go past.
    pub(super) allowed: Vec<publish::Allowed>,
    pub(super) dry_run: bool,
    /// One warning per published, immutable Release that lacks declared
    /// assets.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) notices: Vec<String>,
    /// What the release proof found beyond a pass.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) proof: Vec<registry_check::ProofRow>,
}

impl ReleaseReport {
    /// One `skipped` row per check of the release proof that did not run.
    fn skipped_rows(&self, fields: Fields) -> Fields {
        self.proof.iter().fold(fields, |fields, row| match row {
            registry_check::ProofRow::Skipped { check, variable } => {
                fields.row("skipped", format!("{}, {variable} is unset", check.name()))
            }
            registry_check::ProofRow::Failed { .. } | registry_check::ProofRow::Notice { .. } => {
                fields
            }
        })
    }

    /// `document` with one warning per notice, the release's and the
    /// proof's, and one error per failure of the proof.
    fn with_proof(&self, document: Document) -> Document {
        let mut notices = self.notices.clone();
        let mut failures = Vec::new();
        for row in &self.proof {
            match row {
                registry_check::ProofRow::Failed { check, message } => {
                    failures.push(format!("{}: {message}", check.name()));
                }
                registry_check::ProofRow::Notice { check, message } => {
                    notices.push(format!("{}: {message}", check.name()));
                }
                registry_check::ProofRow::Skipped { .. } => {}
            }
        }
        with_findings(document, &notices, &failures)
    }

    /// One `allowed` row per proof the run went past. The log of such a run
    /// shows each one.
    fn allowed_rows(&self, fields: Fields) -> Fields {
        self.allowed.iter().fold(fields, |fields, allowed| {
            fields.row(
                "allowed",
                format!("{}: {}", allowed.check.name(), allowed.reason),
            )
        })
    }
}

/// A package whose release HEAD still has to make.
#[derive(Serialize)]
pub(super) struct ReleaseRow {
    pub(super) name: String,
    pub(super) version: String,
    pub(super) commit: String,
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

impl Present for ReleaseReport {
    fn present(&self) -> Document {
        match self.outcome {
            publish::Outcome::NothingToRelease => {
                return with_warnings(
                    Document::new().fields(
                        self.allowed_rows(Fields::new().row("outcome", "nothing to release")),
                    ),
                    &self.notices,
                );
            }
            publish::Outcome::Released => {}
        }
        if self.packages.is_empty() && self.releases.is_empty() {
            return with_warnings(
                Document::new().fields(Fields::new().row("no-op", "nothing to publish")),
                &self.notices,
            );
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
        let fields = self.skipped_rows(self.allowed_rows(fields));
        let mut document = Document::new();
        if !self.packages.is_empty() {
            document = document.table(table);
        }
        if fields.is_empty() {
            self.with_proof(document)
        } else {
            self.with_proof(document.fields(fields))
        }
    }

    /// An error when the proof of a dry run failed, after the plan is shown.
    /// `release` itself stops with an error before it reports.
    fn outcome(&self) -> Outcome {
        if self
            .proof
            .iter()
            .any(|row| matches!(row, registry_check::ProofRow::Failed { .. }))
        {
            Outcome::Error
        } else {
            Outcome::Success
        }
    }
}

/// What `release bootstrap` and `release trust` did: one row per crate and
/// step.
#[derive(Serialize)]
pub(super) struct CratesReport {
    pub(super) crates: Vec<first_publish::Row>,
}

impl Present for CratesReport {
    /// A table of the rows, with a `version` column when any row has one and a
    /// `note` column when any row has one, then one error per failed row.
    fn present(&self) -> Document {
        if self.crates.is_empty() {
            return Document::new().fields(Fields::new().row("no-op", "no crate to handle"));
        }
        let versions = self.crates.iter().any(|row| row.version.is_some());
        let notes = self.crates.iter().any(|row| row.note.is_some());
        let mut headers = vec!["name"];
        if versions {
            headers.push("version");
        }
        headers.push("state");
        if notes {
            headers.push("note");
        }
        let table = self
            .crates
            .iter()
            .fold(Table::new(headers).id_column(0), |table, row| {
                let mut cells = vec![row.name.clone()];
                if versions {
                    cells.push(row.version.clone().unwrap_or_else(|| "-".to_owned()));
                }
                cells.push(row.state.label());
                if notes {
                    cells.push(row.note.clone().unwrap_or_default());
                }
                table.row(cells)
            });
        let failures: Vec<String> = self
            .crates
            .iter()
            .filter_map(|row| row.failure().map(str::to_owned))
            .collect();
        with_findings(Document::new().table(table), &[], &failures)
    }

    /// An error when any row failed, after every row is shown.
    fn outcome(&self) -> Outcome {
        if self.crates.iter().any(|row| row.failure().is_some()) {
            Outcome::Error
        } else {
            Outcome::Success
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) prebuilt: Option<String>,
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
        if let Some(path) = &self.prebuilt {
            fields = fields.row("prebuilt", path.clone());
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
    use ctl_core::prelude::{ColorMode, Destination, Outcome, OutputFormat, Present, View};
    use indoc::{formatdoc, indoc};
    use verctl::{publish, registry_check, stop, versions};

    use super::{
        CheckReport, CheckResult, PartialRow, ReleaseReport, ReleaseRow, Report, StatusFragment,
        StatusPackage, StatusReport, Unchecked, VersionCheckReport, VersionReport,
    };

    fn release_report(outcome: publish::Outcome) -> Report {
        Report::Release(ReleaseReport {
            outcome,
            packages: Vec::new(),
            releases: Vec::new(),
            allowed: Vec::new(),
            dry_run: false,
            notices: Vec::new(),
            proof: Vec::new(),
        })
    }

    /// A run that went past a proof shows it on its own row, and the JSON
    /// lists it under `allowed`.
    #[test]
    fn an_allowed_proof_is_on_the_record_in_both_shapes() {
        let report = Report::Release(ReleaseReport {
            outcome: publish::Outcome::NothingToRelease,
            packages: Vec::new(),
            releases: Vec::new(),
            allowed: vec![publish::Allowed {
                check: publish::Check::WorkflowChange,
                reason: ".github/workflows/ at HEAD aaa differs from the default branch at bbb"
                    .into(),
            }],
            dry_run: false,
            notices: Vec::new(),
            proof: Vec::new(),
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
        let report = release_report(publish::Outcome::NothingToRelease);
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

    /// The report lists each check that ran, in both shapes.
    #[test]
    fn check_report_lists_each_check_that_ran() {
        let report = Report::Check(CheckReport {
            checks: vec![
                CheckResult::Fragments { ok: 2 },
                CheckResult::Served,
                CheckResult::Versions(VersionCheckReport {
                    skip: None,
                    rows: Vec::new(),
                    notices: Vec::new(),
                }),
            ],
        });
        let pretty = View::new(OutputFormat::Pretty, ColorMode::Never)
            .width(80)
            .capture(&report)
            .expect("pretty report");
        assert_eq!(pretty.stream(), Destination::Stdout);
        assert_eq!(
            pretty.text(),
            formatdoc! {"
                fragments  ok, 2 fragment(s)
                   served  ok
                 versions  match
            "},
        );

        let json = View::new(OutputFormat::Json, ColorMode::Always)
            .capture(&report)
            .expect("JSON report");
        assert_eq!(json.stream(), Destination::Stdout);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(json.text()).expect("JSON"),
            serde_json::json!({
                "checks": [
                    { "check": "fragments", "ok": 2 },
                    { "check": "served" },
                    { "check": "versions", "skip": null, "rows": [] },
                ]
            })
        );
        assert!(!json.text().contains('\u{1b}'));
    }

    /// A drifted version fails the run, keeps its table and its error, and
    /// still lists the checks that passed.
    #[test]
    fn a_drifted_version_fails_the_check_report() {
        let report = Report::Check(CheckReport {
            checks: vec![
                CheckResult::Served,
                CheckResult::Versions(VersionCheckReport {
                    skip: None,
                    rows: vec![versions::VersionRow {
                        name: "demo".into(),
                        path: "Cargo.toml".into(),
                        local: "1.0.1".into(),
                        remote: Some("1.0.0".into()),
                        released: Some("1.0.0".into()),
                    }],
                    notices: Vec::new(),
                }),
            ],
        });
        assert_eq!(report.outcome(), Outcome::Error);
        let pretty = View::new(OutputFormat::Pretty, ColorMode::Never)
            .width(120)
            .capture(&report)
            .expect("pretty report");
        assert_eq!(pretty.stream(), Destination::Stderr);
        let text = pretty.text();
        assert!(text.contains("  served  ok"), "{text}");
        assert!(text.contains("versions  differ"), "{text}");
        assert!(text.contains("demo 1.0.0 -> 1.0.1"), "{text}");
    }

    /// A registry report with one crate that lacks an owner and one that is
    /// ready: the table shows both, the failure is an error, and the run
    /// fails, in both shapes.
    #[test]
    fn a_registry_report_shows_every_crate_and_fails_on_one() {
        let report = Report::Check(CheckReport {
            checks: vec![CheckResult::Registry(registry_check::RegistryReport {
                crates: vec![
                    registry_check::RegistryRow {
                        name: "demo".into(),
                        held: true,
                        state: registry_check::RegistryState::Present {
                            owners: vec!["alice".into()],
                            expected: Some(vec!["any-victor".into()]),
                            lacking: vec!["any-victor".into()],
                            trusted: registry_check::Evidence::Untrusted,
                        },
                    },
                    registry_check::RegistryRow {
                        name: "demo-core".into(),
                        held: true,
                        state: registry_check::RegistryState::Present {
                            owners: vec!["any-victor".into()],
                            expected: Some(vec!["any-victor".into()]),
                            lacking: Vec::new(),
                            trusted: registry_check::Evidence::ThisRepository {
                                repository: "acme/demo".into(),
                            },
                        },
                    },
                ],
            })],
        });
        assert_eq!(report.outcome(), Outcome::Error);
        let pretty = View::new(OutputFormat::Pretty, ColorMode::Never)
            .width(120)
            .capture(&report)
            .expect("pretty report");
        assert_eq!(pretty.stream(), Destination::Stderr);
        assert_eq!(
            pretty.text(),
            formatdoc! {"
                registry  failed, 1 of 2 crate(s)

                ┌───────────┬─────────────┬────────────┬────────────────────────┐
                │ crate     ┆ state       ┆ owners     ┆ trusted                │
                ╞═══════════╪═════════════╪════════════╪════════════════════════╡
                │ demo      ┆ lacks owner ┆ alice      ┆ not trusted            │
                │ demo-core ┆ ok          ┆ any-victor ┆ trusted from acme/demo │
                └───────────┴─────────────┴────────────┴────────────────────────┘

                error · demo: lacks owner any-victor on crates.io, which lists alice; add it with `cargo owner --add`, or change owners
                in .ctl/ver.yaml
            "}
        );
        let json = View::new(OutputFormat::Json, ColorMode::Never)
            .capture(&report)
            .expect("JSON report");
        assert_eq!(
            json.text(),
            indoc! {r#"
                {
                  "checks": [
                    {
                      "check": "registry",
                      "crates": [
                        {
                          "name": "demo",
                          "held": true,
                          "state": "present",
                          "owners": [
                            "alice"
                          ],
                          "expected": [
                            "any-victor"
                          ],
                          "lacking": [
                            "any-victor"
                          ],
                          "trusted": {
                            "via": "untrusted"
                          }
                        },
                        {
                          "name": "demo-core",
                          "held": true,
                          "state": "present",
                          "owners": [
                            "any-victor"
                          ],
                          "expected": [
                            "any-victor"
                          ],
                          "lacking": [],
                          "trusted": {
                            "via": "this-repository",
                            "repository": "acme/demo"
                          }
                        }
                      ]
                    }
                  ]
                }
            "#}
        );
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
        let report = Report::Release(ReleaseReport {
            outcome: publish::Outcome::Released,
            packages: Vec::new(),
            releases: vec![url.into()],
            allowed: Vec::new(),
            dry_run: false,
            notices: Vec::new(),
            proof: Vec::new(),
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
        let report = Report::Release(ReleaseReport {
            outcome: publish::Outcome::Released,
            packages: Vec::new(),
            releases: vec!["https://example.test/v1".into()],
            allowed: Vec::new(),
            dry_run: false,
            notices: Vec::new(),
            proof: Vec::new(),
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
            assets: None,
            unreleased: Some(vec![ReleaseRow {
                name: "demo".into(),
                version: "0.3.1".into(),
                commit: "aaa".into(),
            }]),
            unreleased_unchecked: None,
            partial: None,
            notices: Vec::new(),
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

                ┌──────┬─────────┬────────┐
                │ name ┆ version ┆ commit │
                ╞══════╪═════════╪════════╡
                │ demo ┆ 0.3.1   ┆ aaa    │
                └──────┴─────────┴────────┘

                   pending  1
                       max  minor
                    assets  none
                unreleased  1
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
                "assets": null,
                "unreleased": [{ "name": "demo", "version": "0.3.1", "commit": "aaa" }],
                "unreleased_unchecked": null,
                "partial": null,
            })
        );
    }

    /// A release that crates.io stopped partway shows its count and the cause
    /// its draft records, in both shapes (task 1.2 of release-order).
    #[test]
    fn a_partial_release_shows_its_count_and_its_cause() {
        let row = |name: &str| ReleaseRow {
            name: name.into(),
            version: "0.4.0".into(),
            commit: "aaa".into(),
        };
        let report = Report::Status(StatusReport {
            pending: 0,
            max: "none".into(),
            fragments: Vec::new(),
            assets: None,
            unreleased: Some(vec![row("core"), row("util"), row("app")]),
            unreleased_unchecked: None,
            partial: Some(vec![PartialRow {
                tag: "v0.4.0".into(),
                live: Some(1),
                total: 3,
                noun: "crates".into(),
                stop: stop::Record::Read(stop::Stop::new(
                    "util",
                    "0.4.0",
                    "429 Too Many Requests",
                    Some("Fri, 13 Nov 2026 19:00:00 GMT".into()),
                )),
            }]),
            notices: Vec::new(),
        });
        let pretty = View::new(OutputFormat::Pretty, ColorMode::Never)
            .width(120)
            .capture(&report)
            .expect("pretty");
        assert_eq!(
            pretty.text(),
            formatdoc! {"
                ┌──────┬─────────┬────────┐
                │ name ┆ version ┆ commit │
                ╞══════╪═════════╪════════╡
                │ core ┆ 0.4.0   ┆ aaa    │
                │ util ┆ 0.4.0   ┆ aaa    │
                │ app  ┆ 0.4.0   ┆ aaa    │
                └──────┴─────────┴────────┘

                   pending  0
                    assets  none
                unreleased  3
                   partial  v0.4.0: partially published: 1 of 3 crates live
                   stopped  util 0.4.0: 429 Too Many Requests, resume after Fri, 13 Nov 2026 19:00:00 GMT (as the last run reported)
            "},
        );
        let json = View::new(OutputFormat::Json, ColorMode::Never)
            .capture(&report)
            .expect("json");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(json.text()).expect("JSON")["partial"],
            serde_json::json!([{
                "tag": "v0.4.0",
                "live": 1,
                "total": 3,
                "noun": "crates",
                "stop": {
                    "state": "read",
                    "package": "util",
                    "version": "0.4.0",
                    "cause": "429 Too Many Requests",
                    "retry_after": "Fri, 13 Nov 2026 19:00:00 GMT"
                }
            }])
        );
    }

    /// A stop record that does not parse is named as unreadable, never
    /// trusted in part.
    #[test]
    fn an_unreadable_stop_record_is_named() {
        let report = Report::Status(StatusReport {
            pending: 0,
            max: "none".into(),
            fragments: Vec::new(),
            assets: None,
            unreleased: Some(Vec::new()),
            unreleased_unchecked: None,
            partial: Some(vec![PartialRow {
                tag: "v0.4.0".into(),
                live: Some(2),
                total: 3,
                noun: "packages".into(),
                stop: stop::Record::Unreadable,
            }]),
            notices: Vec::new(),
        });
        let pretty = View::new(OutputFormat::Pretty, ColorMode::Never)
            .width(120)
            .capture(&report)
            .expect("pretty");
        assert_eq!(
            pretty.text(),
            formatdoc! {"
                   pending  0
                    assets  none
                unreleased  0
                   partial  v0.4.0: partially published: 2 of 3 packages live
                   stopped  stop record unreadable
            "},
        );
    }

    /// An unchecked list is `null` with a reason, never `[]`, so automation
    /// cannot read "not checked" as "nothing pending".
    #[test]
    fn unchecked_unreleased_is_null_with_its_reason() {
        let report = Report::Status(StatusReport {
            pending: 0,
            max: "none".into(),
            fragments: Vec::new(),
            assets: None,
            unreleased: None,
            unreleased_unchecked: Some(Unchecked::missing_token()),
            partial: None,
            notices: Vec::new(),
        });
        let pretty = View::new(OutputFormat::Pretty, ColorMode::Never)
            .width(80)
            .capture(&report)
            .expect("pretty");
        assert_eq!(
            pretty.text(),
            formatdoc! {"
                   pending  0
                    assets  none
                unreleased  not checked (no GITHUB_TOKEN or GH_TOKEN)
            "},
        );
        let json = View::new(OutputFormat::Json, ColorMode::Never)
            .capture(&report)
            .expect("json");
        let value: serde_json::Value = serde_json::from_str(json.text()).expect("JSON");
        assert!(value["unreleased"].is_null());
        assert_eq!(value["unreleased_unchecked"], "no GITHUB_TOKEN or GH_TOKEN");
    }

    /// A failed check shows its top-level message to a person and keeps the
    /// whole chain in JSON.
    #[test]
    fn a_failed_check_shows_its_message_and_keeps_its_chain() {
        let error =
            anyhow::anyhow!("the clone is shallow").context("cannot find the version commit");
        let report = Report::Status(StatusReport {
            pending: 0,
            max: "none".into(),
            fragments: Vec::new(),
            assets: None,
            unreleased: None,
            unreleased_unchecked: Some(Unchecked::failed(&error)),
            partial: None,
            notices: Vec::new(),
        });
        let pretty = View::new(OutputFormat::Pretty, ColorMode::Never)
            .width(80)
            .capture(&report)
            .expect("pretty");
        assert_eq!(
            pretty.text(),
            formatdoc! {"
                   pending  0
                    assets  none
                unreleased  not checked (cannot find the version commit)
            "},
        );
        let json = View::new(OutputFormat::Json, ColorMode::Never)
            .capture(&report)
            .expect("json");
        let value: serde_json::Value = serde_json::from_str(json.text()).expect("JSON");
        assert_eq!(
            value["unreleased_unchecked"],
            "cannot find the version commit: the clone is shallow"
        );
    }
}
