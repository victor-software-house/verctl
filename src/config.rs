use crate::driver::{CommandSpec, Driver, Format};
use crate::publisher::PublisherSpec;
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum CommandField {
    Mise(String),
    Argv(Vec<String>),
}

impl CommandField {
    fn into_spec(self) -> Result<CommandSpec> {
        match self {
            Self::Mise(task) => {
                if task.is_empty() {
                    bail!("mise task name is empty");
                }
                Ok(CommandSpec::Mise(task))
            }
            Self::Argv(argv) => {
                ensure!(!argv.is_empty(), "driver argv is empty");
                Ok(CommandSpec::Argv(argv))
            }
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DriverSpec {
    pub format: Option<String>,
    pub keys: Option<Vec<String>>,
    pub read: Option<CommandField>,
    pub write: Option<CommandField>,
    pub after: Option<String>,
}

impl DriverSpec {
    pub fn into_driver(self, name: &str) -> Result<Driver> {
        if let (Some(read), Some(write)) = (self.read, self.write) {
            return Ok(Driver::Command {
                read: read.into_spec()?,
                write: write.into_spec()?,
                after: self.after,
            });
        }
        let format = match self.format.as_deref() {
            Some("toml") => Format::Toml,
            Some("json") => Format::Json,
            Some(other) => bail!("driver {name:?} has unknown format {other:?}"),
            None => bail!("driver {name:?} needs format+keys or read+write"),
        };
        let keys = self
            .keys
            .filter(|keys| !keys.is_empty())
            .with_context(|| format!("driver {name:?} needs keys"))?;
        Ok(Driver::Path {
            format,
            keys,
            after: self.after,
        })
    }

    fn stock(name: &str) -> Option<Self> {
        match name {
            "cargo" => Some(Self {
                format: Some("toml".into()),
                keys: Some(vec![
                    "workspace.package.version".into(),
                    "package.version".into(),
                ]),
                ..Self::default()
            }),
            "npm" => Some(Self {
                format: Some("json".into()),
                keys: Some(vec!["version".into()]),
                ..Self::default()
            }),
            _ => None,
        }
    }

    fn merge(self, over: Self) -> Self {
        Self {
            format: over.format.or(self.format),
            keys: over.keys.or(self.keys),
            read: over.read.or(self.read),
            write: over.write.or(self.write),
            after: over.after.or(self.after),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PackageSpec {
    pub name: String,
    pub path: PathBuf,
    pub driver: Option<String>,
    /// Stock `cargo` / `bun`, a `[publishers.NAME]` key, or omit to infer.
    pub publisher: Option<String>,
    /// Pretty noun (`crate`, `package`, `wheel`). Defaults from the publisher.
    pub noun: Option<String>,
    /// Override the publisher argv for this package.
    #[serde(default, rename = "publish")]
    pub publish_argv: Option<Vec<String>>,
    pub registry: Option<String>,
    /// Package CHANGELOG.md. Defaults to next to the manifest.
    pub changelog: Option<PathBuf>,
    #[serde(flatten)]
    pub spec: DriverSpec,
}

impl PackageSpec {
    #[must_use]
    pub fn changelog_path(&self, root: &Path) -> PathBuf {
        if let Some(path) = &self.changelog {
            return root.join(path);
        }
        root.join(&self.path)
            .parent()
            .unwrap_or(root)
            .join("CHANGELOG.md")
    }

    pub fn resolve(&self, config: &Config, root: &Path) -> Result<Driver> {
        let inferred = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(stock_name_for_file);
        let driver_name = self.driver.as_deref().or(inferred);
        let mut spec = driver_name
            .and_then(|name| config.driver_spec(name))
            .unwrap_or_default();
        spec = spec.merge(self.spec.clone());
        if spec.after.is_none() {
            spec.after = crate::detect::follow_up(&root.join(&self.path));
        }
        spec.into_driver(driver_name.unwrap_or(self.name.as_str()))
    }
}

/// One machine, declared once as `[runners.NAME]`. The header names it; this
/// is how GitHub finds it. Every label is required at once, so a three-label
/// runner is one machine carrying three labels, not three machines.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Runner {
    pub labels: Vec<String>,
}

/// One job, declared as `[ci.NAME]`. The header names it; `runners` names the
/// machines from `[runners]` it runs on, one check each. No `os`/`arch`/
/// `triple`: validation runs a machine's checks, it does not cross-compile.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Job {
    pub runners: Option<Vec<String>>,
}

/// Native GitHub Release tarballs, plus the recipe every target shares. Omit
/// for libraries. Each `[assets.NAME]` sub-table is one target.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Assets {
    pub bin: Option<String>,
    /// Ran once per target before `build`. Stock rust recipe uses rustup.
    pub prepare: Option<Vec<String>>,
    /// How to produce the binary. Stock rust recipe is `cargo build --release`.
    pub build: Option<Vec<String>>,
    /// Path to the built binary, with `{bin}` `{triple}` `{os}` `{arch}`.
    pub binary: Option<String>,
    /// Retired `targets = [...]` list, captured only so the repos still on it
    /// get the migration instead of a serde type error from the map below.
    /// Remove once qctl and ctl-core have bumped their verctl pin.
    #[serde(default, rename = "targets")]
    pub retired_targets: Option<toml::Value>,
    /// Every remaining sub-table: `[assets.linux-x64]` and friends. A typo'd
    /// recipe key lands here and fails as an unknown target, loudly.
    #[serde(flatten)]
    pub targets: BTreeMap<String, AssetTarget>,
}

/// One build target: the platform it produces and the single machine that
/// builds it. One tarball, one machine — so `runner` is not a list.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct AssetTarget {
    pub runner: Option<String>,
    pub os: Option<String>,
    pub arch: Option<String>,
    pub triple: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub drivers: BTreeMap<String, DriverSpec>,
    pub packages: Vec<PackageSpec>,
    #[serde(default)]
    pub publishers: BTreeMap<String, PublisherSpec>,
    /// Machines, declared once and named. Entirely repo-owned: verctl ships no
    /// built-in names.
    #[serde(default)]
    pub runners: BTreeMap<String, Runner>,
    #[serde(default)]
    pub assets: Option<Assets>,
    /// PR and push validation. Empty means one `verify` on `ubuntu-latest`.
    #[serde(default)]
    pub ci: BTreeMap<String, Job>,
    #[serde(default)]
    pub prepare: Prepare,
    /// Where served-file templates live. Omit for the convention.
    #[serde(default)]
    pub templates: Templates,
    /// Version spellings, named once and listed by the files that carry them,
    /// so a spelling three files share is written once.
    #[serde(default)]
    pub patterns: BTreeMap<String, PinPattern>,
    /// Collocated tool pins rewritten when `package` is bumped.
    #[serde(default)]
    pub pins: Vec<Pin>,
}

/// A file that serves a package version and must track it.
#[derive(Debug, Clone, Deserialize)]
pub struct Pin {
    pub file: PathBuf,
    /// A mise `[tools]` entry, in a file that parses as TOML. Its own
    /// `?ref=v…` includes move with it.
    pub tool: Option<String>,
    /// `[patterns]` ids this file carries, so which file a spelling belongs to
    /// is written down rather than implied by where a table sits.
    #[serde(default, rename = "patterns")]
    pub pattern_ids: Vec<String>,
    /// The patterns those ids name, resolved once when the config is loaded.
    #[serde(skip)]
    pub patterns: Vec<PinPattern>,
    pub package: String,
}

/// Everything verctl owns in a repo lives here, so a repo root gains one
/// entry instead of a scatter of tool files.
pub const DIR: &str = ".verctl";

/// Where the templates for served files live, and how they are marked. The
/// defaults are the whole convention: a repo that follows it writes nothing.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Templates {
    /// The source tree mirroring the repo. A template's path under it is its
    /// target, so nothing sits beside the file it generates.
    #[serde(default = "Templates::default_source")]
    pub source: PathBuf,
    /// What marks a source file as a template of its target.
    #[serde(default = "Templates::default_suffix")]
    pub suffix: String,
}

impl Templates {
    fn default_source() -> PathBuf {
        Path::new(DIR).join("templates")
    }

    fn default_suffix() -> String {
        ".jinja".to_owned()
    }
}

impl Default for Templates {
    fn default() -> Self {
        Self {
            source: Self::default_source(),
            suffix: Self::default_suffix(),
        }
    }
}

/// One version spelling in a file, and how often the file may say it.
#[derive(Debug, Clone, Deserialize)]
pub struct PinPattern {
    /// The text around a version, `{version}` where the version is. Literal,
    /// not a regex, so nothing about the file's format is assumed.
    pub r#match: String,
    /// How often the file must say it. A pin that lost its only mention, or
    /// drifted to one nobody declared, fails the release rather than serving
    /// a stale version.
    #[serde(default)]
    pub occurrences: Occurrences,
}

/// How many times a pattern must match. A count is not always the useful
/// shape: a document whose examples come and go needs "one or more", and a
/// spelling that was retired needs "none".
#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Occurrences {
    /// Exactly one. What a pin normally is, so it is the default.
    #[default]
    Once,
    /// One or more, however many. For a file whose mentions grow and shrink
    /// on their own, where only "still tracked" is worth asserting.
    Many,
    /// None. A spelling that must stay gone, so its return fails a release
    /// instead of being served.
    Never,
    /// Exactly this many, for a file that says it a known number of times.
    Exactly(usize),
    /// This many or more.
    AtLeast(usize),
}

impl Occurrences {
    #[must_use]
    pub const fn allows(self, found: usize) -> bool {
        match self {
            Self::Once => found == 1,
            Self::Many => found >= 1,
            Self::Never => found == 0,
            Self::Exactly(count) => found == count,
            Self::AtLeast(count) => found >= count,
        }
    }
}

impl fmt::Display for Occurrences {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Once => formatter.write_str("exactly once"),
            Self::Many => formatter.write_str("one or more"),
            Self::Never => formatter.write_str("never"),
            Self::Exactly(count) => write!(formatter, "exactly {count} times"),
            Self::AtLeast(count) => write!(formatter, "{count} or more times"),
        }
    }
}

/// Extra work after version bumps, committed on the Version PR.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Prepare {
    /// argv after all bumps (e.g. `["mise", "run", "version-sync"]`).
    pub after: Option<Vec<String>>,
    /// Extra paths `after` may write. Anything else dirty fails.
    #[serde(default)]
    pub stage: Vec<String>,
    /// Collect gitignored paths that match `stage`. Off by default.
    #[serde(default)]
    pub stage_ignored: bool,
    /// Label `prepare --pr` applies. `check --versions` trusts this
    /// on the GitHub event, not `GITHUB_HEAD_REF`.
    pub version_label: Option<String>,
}

impl Prepare {
    pub const DEFAULT_VERSION_LABEL: &'static str = "verctl:version";

    #[must_use]
    pub fn version_label(&self) -> &str {
        self.version_label
            .as_deref()
            .filter(|label| !label.is_empty())
            .unwrap_or(Self::DEFAULT_VERSION_LABEL)
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path).with_context(|| path.display().to_string())?;
        let mut config: Self =
            toml::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
        if config.packages.is_empty() {
            bail!("{} has no [[packages]]", path.display());
        }
        config
            .resolve_patterns()
            .with_context(|| format!("parse {}", path.display()))?;
        Ok(config)
    }

    /// Turn every pin's `[patterns]` ids into the patterns they name, so no
    /// consumer looks an id up again — and a name that does not resolve, or
    /// resolves for nobody, stops the release here.
    fn resolve_patterns(&mut self) -> Result<()> {
        let declared = &self.patterns;
        let mut referenced: BTreeSet<&str> = BTreeSet::new();
        for pin in &mut self.pins {
            let mut listed: BTreeSet<&str> = BTreeSet::new();
            for id in &pin.pattern_ids {
                if !listed.insert(id.as_str()) {
                    bail!("pin {} lists pattern {id:?} twice", pin.file.display());
                }
                let pattern = declared
                    .get(id)
                    .with_context(|| format!("pin {}: no [patterns.{id}]", pin.file.display()))?;
                pin.patterns.push(pattern.clone());
            }
            referenced.extend(listed);
            if pin.tool.is_none() && pin.patterns.is_empty() {
                bail!(
                    "pin {} needs a tool or a pattern to rewrite",
                    pin.file.display()
                );
            }
        }
        for id in declared.keys() {
            if !referenced.contains(id.as_str()) {
                bail!("pattern {id:?} is declared and no pin lists it");
            }
        }
        Ok(())
    }

    pub fn find(&self, name: &str) -> Result<&PackageSpec> {
        self.packages
            .iter()
            .find(|package| package.name == name)
            .with_context(|| format!("package {name:?} is not in verctl config"))
    }

    #[must_use]
    pub fn driver_spec(&self, name: &str) -> Option<DriverSpec> {
        match (DriverSpec::stock(name), self.drivers.get(name)) {
            (Some(stock), Some(over)) => Some(stock.merge(over.clone())),
            (Some(stock), None) => Some(stock),
            (None, Some(over)) => Some(over.clone()),
            (None, None) => None,
        }
    }
}

fn stock_name_for_file(file_name: &str) -> Option<&'static str> {
    match file_name {
        "Cargo.toml" => Some("cargo"),
        "package.json" => Some("npm"),
        _ => None,
    }
}

pub(crate) fn stock_driver(name: &str) -> Result<Driver> {
    DriverSpec::stock(name)
        .with_context(|| format!("unknown stock driver {name:?}"))?
        .into_driver(name)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use indoc::{formatdoc, indoc};

    /// A whole config, so what is under test is what a repo actually writes.
    fn load(body: &str) -> Result<Config> {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("verctl.toml");
        fs::write(&path, body).unwrap();
        Config::load(&path)
    }

    /// The one package every fixture below needs, and nothing else.
    const PACKAGE: &str = indoc! {r#"
        [[packages]]
        name = "demo"
        path = "Cargo.toml"
    "#};

    /// Every arity a repo can write, in the spelling it writes it.
    #[test]
    fn every_occurrence_form_parses_and_means_what_it_says() {
        let config = load(&formatdoc! {r#"
            {PACKAGE}
            [patterns.omitted]
            match = "a@{{version}}"

            [patterns.once]
            match = "b@{{version}}"
            occurrences = "once"

            [patterns.many]
            match = "c@{{version}}"
            occurrences = "many"

            [patterns.never]
            match = "d@{{version}}"
            occurrences = "never"

            [patterns.twice]
            match = "e@{{version}}"
            occurrences = {{ exactly = 2 }}

            [patterns.floor]
            match = "f@{{version}}"
            occurrences = {{ at_least = 2 }}

            [[pins]]
            file = "README.md"
            package = "demo"
            patterns = ["omitted", "once", "many", "never", "twice", "floor"]
        "#})
        .unwrap();
        let allowed: Vec<Vec<usize>> = config.pins[0]
            .patterns
            .iter()
            .map(|pattern| (0..4).filter(|n| pattern.occurrences.allows(*n)).collect())
            .collect();
        assert_eq!(
            allowed,
            [
                vec![1],       // omitted: exactly once
                vec![1],       // once
                vec![1, 2, 3], // many
                vec![0],       // never
                vec![2],       // exactly 2
                vec![2, 3],    // at least 2
            ]
        );
    }

    #[test]
    fn an_unknown_arity_is_not_silently_a_default() {
        let broken = load(&formatdoc! {r#"
            {PACKAGE}
            [patterns.wrong]
            match = "a@{{version}}"
            occurrences = "sometimes"

            [[pins]]
            file = "README.md"
            package = "demo"
            patterns = ["wrong"]
        "#});
        assert!(broken.is_err());
    }

    /// One spelling, named once, carried by two files: what ids are for.
    #[test]
    fn one_named_pattern_serves_every_file_that_lists_it() {
        let config = load(&formatdoc! {r#"
            {PACKAGE}
            [patterns.install]
            match = "demo@{{version}}"

            [patterns.mention]
            match = "v{{version}}"
            occurrences = "many"

            [[pins]]
            file = "README.md"
            package = "demo"
            patterns = ["install", "mention"]

            [[pins]]
            file = "docs/install.md"
            package = "demo"
            patterns = ["install"]
        "#})
        .unwrap();
        let carried: Vec<Vec<&str>> = config
            .pins
            .iter()
            .map(|pin| {
                pin.patterns
                    .iter()
                    .map(|pattern| pattern.r#match.as_str())
                    .collect()
            })
            .collect();
        assert_eq!(
            carried,
            [vec!["demo@{version}", "v{version}"], vec!["demo@{version}"]],
            "each file carries the patterns it lists, in the order it lists them"
        );
    }

    /// The id system's own failures, each naming what a repo has to fix.
    #[test]
    fn a_reference_that_does_not_resolve_stops_the_load() {
        let cases = [
            (
                "an id no [patterns] declares",
                formatdoc! {r#"
                    {PACKAGE}
                    [patterns.install]
                    match = "demo@{{version}}"

                    [[pins]]
                    file = "README.md"
                    package = "demo"
                    patterns = ["install", "instal"]
                "#},
                "pin README.md: no [patterns.instal]",
            ),
            (
                "the same id listed twice",
                formatdoc! {r#"
                    {PACKAGE}
                    [patterns.install]
                    match = "demo@{{version}}"

                    [[pins]]
                    file = "README.md"
                    package = "demo"
                    patterns = ["install", "install"]
                "#},
                r#"pin README.md lists pattern "install" twice"#,
            ),
            (
                "a pattern no pin lists",
                formatdoc! {r#"
                    {PACKAGE}
                    [patterns.install]
                    match = "demo@{{version}}"

                    [patterns.orphan]
                    match = "old@{{version}}"

                    [[pins]]
                    file = "README.md"
                    package = "demo"
                    patterns = ["install"]
                "#},
                r#"pattern "orphan" is declared and no pin lists it"#,
            ),
            (
                "a pin with nothing to rewrite",
                formatdoc! {r#"
                    {PACKAGE}
                    [[pins]]
                    file = "README.md"
                    package = "demo"
                "#},
                "pin README.md needs a tool or a pattern to rewrite",
            ),
        ];
        for (scenario, body, expected) in cases {
            let error = load(&body).expect_err(scenario);
            assert!(
                format!("{error:#}").contains(expected),
                "{scenario}: {error:#}"
            );
        }
    }
}
