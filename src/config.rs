use crate::driver::{CommandSpec, Driver, Format};
use crate::publisher::PublisherSpec;
use crate::schema::{at_least_one, cannot_be_empty, inside_the_repo};
use anyhow::{Context, Result, bail, ensure};
use garde::Validate;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
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
    /// Stock `cargo` / `bun`, a `publishers` key, or omit to infer.
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

/// One machine, declared once under `runners`. The key names it; this
/// is how GitHub finds it. Every label is required at once, so a three-label
/// runner is one machine carrying three labels, not three machines.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Validate)]
#[serde(deny_unknown_fields)]
#[garde(context(Config))]
pub struct Runner {
    /// Every label at once. A machine with none cannot be found at all.
    #[garde(custom(at_least_one("label")))]
    pub labels: Vec<String>,
}

/// One job, declared under `ci`. The key names it; `runners` names the
/// machines from `runners` it runs on, one check each. No `os`/`arch`/
/// `triple`: validation runs a machine's checks, it does not cross-compile.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Job {
    pub runners: Option<Vec<String>>,
}

/// Native GitHub Release tarballs, plus the recipe every target shares. Omit
/// for libraries. Each named sub-mapping is one target.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Assets {
    pub bin: Option<String>,
    /// Ran once per target before `build`. Stock rust recipe uses rustup.
    pub prepare: Option<Vec<String>>,
    /// How to produce the binary. Stock rust recipe is `cargo build --release`.
    pub build: Option<Vec<String>>,
    /// Path to the built binary, with `{bin}` `{triple}` `{os}` `{arch}`.
    pub binary: Option<String>,
    /// Every remaining sub-mapping: `assets.linux-x64` and friends. A typo'd
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

/// The whole file, and the context every rule in it is checked against: a
/// pattern is only unused, and an id only unknown, relative to the rest of the
/// config. `Config::parse` reads the shape with serde and then validates it
/// once, here, so a complaint names the field that has to change.
#[derive(Debug, Clone, Deserialize, Default, Validate)]
#[garde(context(Self as config))]
#[garde(allow_unvalidated)]
pub struct Config {
    #[serde(default)]
    pub drivers: BTreeMap<String, DriverSpec>,
    /// What this repo releases. A config that names none has nothing to do.
    #[garde(custom(at_least_one("package")))]
    pub packages: Vec<PackageSpec>,
    #[serde(default)]
    pub publishers: BTreeMap<String, PublisherSpec>,
    /// Machines, declared once and named. Entirely repo-owned: verctl ships no
    /// built-in names.
    #[serde(default)]
    #[garde(dive)]
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
    #[garde(dive)]
    pub templates: Templates,
    /// Version spellings, named once and listed by the files that carry them,
    /// so a spelling three files share is written once.
    #[serde(default)]
    #[garde(custom(every_pattern_says_where_the_version_goes))]
    #[garde(custom(every_pattern_is_listed))]
    pub patterns: BTreeMap<String, PinPattern>,
    /// Collocated tool pins rewritten when `package` is bumped.
    #[serde(default)]
    #[garde(dive)]
    pub pins: Vec<Pin>,
}

/// Where the version goes, said once: no placeholder is a spelling that tracks
/// nothing, two is a spelling with no single version to put anywhere.
fn every_pattern_says_where_the_version_goes(
    patterns: &BTreeMap<String, PinPattern>,
    _: &Config,
) -> garde::Result {
    for (id, pattern) in patterns {
        let found = pattern.r#match.matches(PLACEHOLDER).count();
        if found != 1 {
            return Err(garde::Error::new(format!(
                "{id}: match says {PLACEHOLDER} {found} times, and must say it once"
            )));
        }
    }
    Ok(())
}

/// A declared pattern nothing lists is dead configuration, not a spare: it
/// would sit there reading like a tracked spelling while no file is checked
/// against it.
fn every_pattern_is_listed(
    patterns: &BTreeMap<String, PinPattern>,
    config: &Config,
) -> garde::Result {
    let listed: BTreeSet<&str> = config
        .pins
        .iter()
        .flat_map(|pin| pin.pattern_ids.iter().map(String::as_str))
        .collect();
    for id in patterns.keys() {
        if !listed.contains(id.as_str()) {
            return Err(garde::Error::new(format!(
                "{id:?} is declared and no pin lists it"
            )));
        }
    }
    Ok(())
}

/// A file that serves a package version and must track it.
#[derive(Debug, Clone, Deserialize, Validate)]
#[garde(context(Config as config))]
#[garde(allow_unvalidated)]
pub struct Pin {
    #[garde(custom(inside_the_repo))]
    pub file: PathBuf,
    /// A mise `[tools]` entry, in a file that parses as TOML. Its own
    /// `?ref=v…` includes move with it.
    pub tool: Option<String>,
    /// `patterns` ids this file carries, so which file a spelling belongs to
    /// is written down rather than implied by where a table sits.
    #[serde(default, rename = "patterns")]
    #[garde(
        custom(each_id_is_declared),
        custom(no_id_is_listed_twice),
        custom(something_to_rewrite(self.tool.is_some()))
    )]
    pub pattern_ids: Vec<String>,
    /// The patterns those ids name, filled in by `Config::load` — the only way
    /// to build a pin that rewrites anything.
    #[serde(skip)]
    pub patterns: Vec<PinPattern>,
    pub package: String,
}

/// A name is only a name if something declares it.
fn each_id_is_declared(ids: &[String], config: &Config) -> garde::Result {
    for id in ids {
        if !config.patterns.contains_key(id) {
            return Err(garde::Error::new(format!("no patterns.{id}")));
        }
    }
    Ok(())
}

/// Listing a spelling twice says nothing twice: arity belongs to the pattern.
fn no_id_is_listed_twice(ids: &[String], _: &Config) -> garde::Result {
    let mut seen = BTreeSet::new();
    for id in ids {
        if !seen.insert(id.as_str()) {
            return Err(garde::Error::new(format!("lists {id:?} twice")));
        }
    }
    Ok(())
}

/// A pin with neither a `tool` nor a pattern names a file and asks for nothing.
fn something_to_rewrite(has_tool: bool) -> impl Fn(&[String], &Config) -> garde::Result {
    move |ids, _| {
        if has_tool || !ids.is_empty() {
            return Ok(());
        }
        Err(garde::Error::new("needs a tool or a pattern to rewrite"))
    }
}

/// The directory every ctl CLI shares, so a repo root gains one entry however
/// many of them it uses. verctl owns the files it names here and nothing else.
pub const DIR: &str = ".ctl";

/// verctl's declarations, named after the task an operator runs (`mise run
/// ver`) rather than after the crate that reads them.
pub const FILE: &str = ".ctl/ver.yaml";

/// The directory a config governs — what every `path` in it is relative to.
///
/// Declarations sit in `<root>/.ctl/`, so the directory holding the file is a
/// level too deep and its parent is the root. `-c crates/foo/.ctl/ver.yaml`
/// therefore still governs `crates/foo`. A file outside a `.ctl` directory is
/// read where it sits, which is what an ad-hoc `-c` asks for.
#[must_use]
pub fn root_of(config: &Path) -> PathBuf {
    let dir = here(config.parent());
    if dir.file_name() == Some(OsStr::new(DIR)) {
        return here(dir.parent()).to_path_buf();
    }
    dir.to_path_buf()
}

fn here(path: Option<&Path>) -> &Path {
    path.filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

/// What a `pins` pattern puts where the version goes.
pub const PLACEHOLDER: &str = "{version}";

/// Where the templates for served files live, and how they are marked. The
/// defaults are the whole convention: a repo that follows it writes nothing.
#[derive(Debug, Clone, Deserialize, Validate)]
#[serde(deny_unknown_fields)]
#[garde(context(Config))]
pub struct Templates {
    /// The directory every template lives in, flat. Each one declares its own
    /// target, so nothing sits beside the file it generates.
    #[serde(default = "Templates::default_source")]
    #[garde(custom(inside_the_repo))]
    pub source: PathBuf,
    /// What marks a source file as a template of its target.
    #[serde(default = "Templates::default_suffix")]
    #[garde(custom(cannot_be_empty(
        "it is what marks a file in the source tree as a template, like \".jinja\""
    )))]
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
    /// not a regex, so nothing about the file's format is assumed. Exactly one
    /// placeholder, checked over `patterns` so the complaint names the id.
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
#[derive(Debug, Clone, Copy, Default)]
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

/// The two shapes an arity is written in: a word on its own, or a bound with
/// its number. Written out because a bare `usize` variant would deserialize
/// from a YAML tag (`!exactly 2`), and a repo writes a mapping.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum DeclaredOccurrences {
    Word(OccurrenceWord),
    Exactly { exactly: usize },
    AtLeast { at_least: usize },
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum OccurrenceWord {
    Once,
    Many,
    Never,
}

impl<'de> Deserialize<'de> for Occurrences {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match DeclaredOccurrences::deserialize(deserializer)? {
            DeclaredOccurrences::Word(OccurrenceWord::Once) => Self::Once,
            DeclaredOccurrences::Word(OccurrenceWord::Many) => Self::Many,
            DeclaredOccurrences::Word(OccurrenceWord::Never) => Self::Never,
            DeclaredOccurrences::Exactly { exactly } => Self::Exactly(exactly),
            DeclaredOccurrences::AtLeast { at_least } => Self::AtLeast(at_least),
        })
    }
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
        Self::parse(&raw).with_context(|| path.display().to_string())
    }

    /// The one parse boundary. `load` adds the file name; everything else —
    /// tests included — comes through here, so a document is read exactly one
    /// way and a complaint is worded exactly once.
    pub fn parse(raw: &str) -> Result<Self> {
        let mut config: Self = yaml_serde::from_str(raw).context("parse")?;
        config.validate_with(&config)?;
        config.resolve_patterns();
        Ok(config)
    }

    /// Turn every pin's `patterns` ids into the patterns they name, so no
    /// consumer looks an id up again. Validation already proved every name
    /// resolves, which is why this cannot fail.
    fn resolve_patterns(&mut self) {
        let declared = self.patterns.clone();
        for pin in &mut self.pins {
            pin.patterns = pin
                .pattern_ids
                .iter()
                .filter_map(|id| declared.get(id).cloned())
                .collect();
        }
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
        Config::parse(body)
    }

    /// The one package every fixture below needs, and nothing else.
    const PACKAGE: &str = indoc! {"
        packages:
          - name: demo
            path: Cargo.toml
    "};

    /// A `path` is relative to the repository, not to the directory the
    /// declarations happen to sit in, so `.ctl` never lands in front of it.
    #[test]
    fn the_root_is_the_directory_holding_the_ctl_dir() {
        let cases = [
            (".ctl/ver.yaml", "."),
            ("crates/foo/.ctl/ver.yaml", "crates/foo"),
            ("/srv/repo/.ctl/ver.yaml", "/srv/repo"),
            ("ver.yaml", "."),
            ("/tmp/scratch/ver.yaml", "/tmp/scratch"),
        ];
        for (config, expected) in cases {
            assert_eq!(root_of(Path::new(config)), Path::new(expected), "{config}");
        }
    }

    /// Every arity a repo can write, in the spelling it writes it.
    #[test]
    fn every_occurrence_form_parses_and_means_what_it_says() {
        let config = load(&formatdoc! {r#"
            {PACKAGE}
            patterns:
              omitted:
                match: "a@{{version}}"
              once:
                match: "b@{{version}}"
                occurrences: once
              many:
                match: "c@{{version}}"
                occurrences: many
              never:
                match: "d@{{version}}"
                occurrences: never
              twice:
                match: "e@{{version}}"
                occurrences: {{exactly: 2}}
              floor:
                match: "f@{{version}}"
                occurrences: {{at_least: 2}}

            pins:
              - file: README.md
                package: demo
                patterns: [omitted, once, many, never, twice, floor]
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
            patterns:
              wrong:
                match: "a@{{version}}"
                occurrences: sometimes

            pins:
              - file: README.md
                package: demo
                patterns: [wrong]
        "#});
        assert!(broken.is_err());
    }

    /// One spelling, named once, carried by two files: what ids are for.
    #[test]
    fn one_named_pattern_serves_every_file_that_lists_it() {
        let config = load(&formatdoc! {r#"
            {PACKAGE}
            patterns:
              install:
                match: "demo@{{version}}"
              mention:
                match: "v{{version}}"
                occurrences: many

            pins:
              - file: README.md
                package: demo
                patterns: [install, mention]
              - file: docs/install.md
                package: demo
                patterns: [install]
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

    /// Every rule the schema declares, each naming what a repo has to fix.
    #[test]
    #[allow(clippy::too_many_lines)] // one case per rule; a table, not logic
    fn a_config_that_breaks_a_rule_stops_the_load() {
        let cases = [
            (
                "an id no patterns section declares",
                formatdoc! {r#"
                    {PACKAGE}
                    patterns:
                      install:
                        match: "demo@{{version}}"

                    pins:
                      - file: README.md
                        package: demo
                        patterns: [install, instal]
                "#},
                "pins[0].pattern_ids: no patterns.instal",
            ),
            (
                "the same id listed twice",
                formatdoc! {r#"
                    {PACKAGE}
                    patterns:
                      install:
                        match: "demo@{{version}}"

                    pins:
                      - file: README.md
                        package: demo
                        patterns: [install, install]
                "#},
                r#"pins[0].pattern_ids: lists "install" twice"#,
            ),
            (
                "a pattern no pin lists",
                formatdoc! {r#"
                    {PACKAGE}
                    patterns:
                      install:
                        match: "demo@{{version}}"
                      orphan:
                        match: "old@{{version}}"

                    pins:
                      - file: README.md
                        package: demo
                        patterns: [install]
                "#},
                r#"patterns: "orphan" is declared and no pin lists it"#,
            ),
            (
                "a pin with nothing to rewrite",
                formatdoc! {"
                    {PACKAGE}
                    pins:
                      - file: README.md
                        package: demo
                "},
                "pins[0].pattern_ids: needs a tool or a pattern to rewrite",
            ),
            (
                "a spelling with nowhere to put the version",
                formatdoc! {r#"
                    {PACKAGE}
                    patterns:
                      install:
                        match: "demo@1.2.3"

                    pins:
                      - file: README.md
                        package: demo
                        patterns: [install]
                "#},
                "patterns: install: match says {version} 0 times",
            ),
            (
                "a pin reaching out of the repository",
                formatdoc! {r#"
                    {PACKAGE}
                    patterns:
                      install:
                        match: "demo@{{version}}"

                    pins:
                      - file: ../elsewhere/README.md
                        package: demo
                        patterns: [install]
                "#},
                "pins[0].file: must stay inside the repository",
            ),
            (
                "a machine no label can find",
                formatdoc! {"
                    {PACKAGE}
                    runners:
                      ghost:
                        labels: []
                "},
                "runners.ghost.labels: must declare at least one label",
            ),
            (
                "a suffix that makes every file a template",
                formatdoc! {r#"
                    {PACKAGE}
                    templates:
                      suffix: ""
                "#},
                "templates.suffix: cannot be empty — it is what marks a file",
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
