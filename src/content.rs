//! What a Version PR may change: what the version step writes from the
//! fragments it consumes, and nothing else.
//!
//! The check replays the version step on the merge-base tree in a scratch
//! directory. The replay plans the fragments there, writes each manifest
//! through its driver, applies each pin, prepends each CHANGELOG section, and
//! removes each consumed fragment. It compares the result with the working
//! tree exactly.
//!
//! The replay runs no lockfile owner and no `prepare.after` command, and
//! reaches no network, so it gives the same answer with or without a
//! registry. The check reads a lockfile through [`crate::lockfile`] instead:
//! only the workspace versions the plan moves may change in it.
//!
//! Each served file must equal its template's render at the versions on HEAD.
//! Any path that `prepare.stage` names at the merge-base may change, since
//! `prepare.after` writes it.
//! A lockfile verctl parses is the exception: only a workspace version the
//! plan moves may change in it.
//! The Version PR's review covers the rest of the content.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde_json::Value;
use toml_edit::{DocumentMut, Item};

use crate::config::Config;
use crate::git::{self, MergeBase};
use crate::lockfile::{self, Change, Format, Kind, Lockfile};
use crate::prepare::{self, PlanEntry};
use crate::{detect, fragment, members, pins, release, templates};

/// The marker of an error that came from loading the config at the
/// merge-base, which a caller may skip a rule over instead of failing.
#[derive(Debug)]
pub struct BaseConfigFailed;

impl std::fmt::Display for BaseConfigFailed {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("the config at the merge-base does not load")
    }
}

/// The most keys a manifest difference names.
const KEYS_NAMED: usize = 8;

/// The files `check --versions` reads, as its command line names them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inputs {
    pub(crate) config: PathBuf,
    pub(crate) dir: PathBuf,
}

impl Inputs {
    /// The config file and the fragment directory, each as given: relative to
    /// the working directory, or absolute.
    #[must_use]
    pub fn new(config: impl Into<PathBuf>, dir: impl Into<PathBuf>) -> Self {
        Self {
            config: config.into(),
            dir: dir.into(),
        }
    }
}

/// Fail unless the Version PR changes exactly what the version step writes
/// from the merge-base.
///
/// `root` is the directory holding `.ctl/`, and `config` the config on HEAD,
/// which names the served files. The plan, the pins, and `prepare.stage` come
/// from the config at the merge-base, so a Version PR cannot widen what it may
/// change. Nothing is checked when there is no repository, or no `origin` and
/// none of `candidates`.
///
/// When the config at the merge-base does not load, as when this pull request
/// removes a key the running verctl refuses, the replay has no plan to run.
/// The check is then skipped, and the notice names it and the load error.
///
/// # Errors
///
/// Returns an error listing every difference found:
///
/// - a path the version step does not write;
/// - a manifest, pin, CHANGELOG, or fragment that differs from the replay;
/// - a served file that differs from its render;
/// - a lockfile change other than a planned workspace version.
///
/// It also returns one when the merge-base cannot be read (see
/// [`MergeBase::read`]), or when the fragments there cannot be read or
/// planned. The same holds when the replay cannot be written, or when
/// `cargo metadata` fails on a planned cargo manifest.
pub fn check(
    root: &Path,
    config: &Config,
    inputs: &Inputs,
    candidates: &[String],
) -> Result<Vec<String>> {
    let Some(base) = MergeBase::read(root, candidates)? else {
        return Ok(Vec::new());
    };
    let problems = match problems(root, config, inputs, &base) {
        Ok(problems) => problems,
        Err(error) if error.downcast_ref::<BaseConfigFailed>().is_some() => {
            return Ok(vec![format!(
                "the Version PR content check is skipped: {error:#}"
            )]);
        }
        Err(error) => return Err(error),
    };
    if problems.is_empty() {
        return Ok(Vec::new());
    }
    bail!(
        "the Version PR differs from what the version step writes at the merge-base in {count} place(s); run `verctl version --pr` to rewrite it:\n{list}",
        count = problems.len(),
        list = problems.join("\n"),
    );
}

/// Whether the working tree deletes a fragment that exists at `base`, as a
/// Version PR does when it consumes one. A Version PR that bumps no version
/// still consumes its fragments, so `check --versions` asks whether such a
/// pull request is the Version PR.
///
/// # Errors
///
/// Returns an error when the fragment directory cannot be placed inside the
/// working tree.
pub fn removes_fragment(base: &MergeBase, inputs: &Inputs) -> Result<bool> {
    let dir = inside(base.workdir(), &inputs.dir)?;
    Ok(base.changed().iter().any(|path| {
        let path = Path::new(path);
        path.parent() == Some(dir.as_path())
            && fragment::is_fragment_path(path)
            && !base.workdir().join(path).exists()
    }))
}

fn problems(root: &Path, head: &Config, inputs: &Inputs, base: &MergeBase) -> Result<Vec<String>> {
    let scratch = Scratch::new()?;
    base.write_to(scratch.path())?;
    let workdir = base.workdir();
    let replay = Replay::run(scratch.path(), workdir, root, inputs)?;
    let mut problems = replay.problems.clone();
    let root = root.canonicalize().context("the config root")?;
    let served = served(&root, head, workdir, &mut problems)?;
    let mut locks = Vec::new();
    let paths: BTreeSet<&String> = base.changed().iter().chain(&replay.written).collect();
    for path in paths {
        if replay.written.contains(path) {
            let expected = git::read_optional(&scratch.path().join(path))?;
            let actual = base.worktree_bytes(path)?;
            if expected != actual {
                problems.push(describe(path, expected.as_deref(), actual.as_deref()));
            }
        } else if !served.contains(path) {
            let staged = replay
                .stage
                .iter()
                .any(|pattern| git::matches_stage(pattern, path));
            match lockfile::kind(Path::new(path)) {
                Some(kind @ Kind::Parsed(_)) => locks.push((path.as_str(), kind)),
                _ if staged => {}
                Some(kind) => locks.push((path.as_str(), kind)),
                None => problems.push(format!("{path}: the version step does not write this file")),
            }
        }
    }
    problems.extend(lock_changes(base, &replay.moved, &locks)?);
    problems.extend(stale_locks(base, &replay.moved)?);
    Ok(problems)
}

/// The version step replayed on the merge-base tree.
struct Replay {
    /// Every path the replay wrote or removed, relative to the working tree.
    written: BTreeSet<String>,
    /// `prepare.stage` at the merge-base.
    stage: Vec<String>,
    /// Every workspace version the plan moves.
    moved: Vec<Moved>,
    /// What the replay could not apply, such as a missing CHANGELOG section.
    problems: Vec<String>,
}

/// A workspace version the plan moves, directly or through workspace
/// inheritance.
struct Moved {
    /// The manifest that declares it, or the planned manifest whose workspace
    /// version it inherits, relative to the working tree.
    manifest: String,
    /// The names a lockfile may record it under: the configured name and the
    /// name its manifest declares.
    names: BTreeSet<String>,
    from: String,
    to: String,
}

impl Moved {
    /// The directory a JavaScript lockfile at `lock_dir` names this member
    /// by: `.` for the lockfile's own directory.
    fn label(&self, lock_dir: &Path) -> Option<String> {
        let dir = Path::new(&self.manifest).parent()?;
        let label = dir.strip_prefix(lock_dir).ok()?;
        Some(if label.as_os_str().is_empty() {
            ".".to_owned()
        } else {
            label.display().to_string()
        })
    }

    /// Whether `entry` in the `format` lockfile at `lock_dir` records this
    /// member.
    fn records(&self, format: Format, lock_dir: &Path, name: &str, label: &str) -> bool {
        match format {
            Format::Cargo => self.names.contains(name),
            Format::Uv => {
                let name = pep503(name);
                self.names.iter().any(|known| pep503(known) == name)
            }
            Format::Bun | Format::Npm => self.label(lock_dir).as_deref() == Some(label),
        }
    }
}

/// A Python project name as uv writes it in `uv.lock`: lowercase, with each
/// run of `-`, `_`, and `.` as one `-` ([PEP 503]).
///
/// [PEP 503]: https://peps.python.org/pep-0503/#normalized-names
fn pep503(name: &str) -> String {
    let mut normalized = String::with_capacity(name.len());
    for character in name.chars() {
        if matches!(character, '-' | '_' | '.') {
            if !normalized.ends_with('-') {
                normalized.push('-');
            }
        } else {
            normalized.extend(character.to_lowercase());
        }
    }
    normalized
}

impl Replay {
    fn run(scratch: &Path, workdir: &Path, root: &Path, inputs: &Inputs) -> Result<Self> {
        let replay_root = scratch.join(inside(workdir, root)?);
        let config = Config::load(&scratch.join(inside(workdir, &inputs.config)?))
            .context(BaseConfigFailed)?;
        let fragments = fragment::load_dir(&scratch.join(inside(workdir, &inputs.dir)?))?;
        let plan = prepare::plan(&config, &fragments, &replay_root)
            .context("the plan of the fragments at the merge-base")?;
        let consumed = release::contributing_fragments(&plan, &fragments);
        let planned: Vec<(String, String)> = plan
            .iter()
            .map(|entry| (entry.name.clone(), entry.to.clone()))
            .collect();
        let cargo = cargo_manifests(&plan, scratch, workdir)?;
        let before = cargo_versions(&cargo)?;
        prepare::apply_plan(&plan)?;
        let after = cargo_versions(&cargo)?;
        let mut written = BTreeSet::new();
        for entry in &plan {
            written.insert(relative(&entry.path, scratch)?);
        }
        for path in pins::write(&replay_root, &config.pins, &planned)? {
            written.insert(relative(&path, scratch)?);
        }
        let mut problems = Vec::new();
        for entry in &plan {
            let path = config.find(&entry.name)?.changelog_path(&replay_root);
            let rel = relative(&path, scratch)?;
            let head = fs::read_to_string(workdir.join(&rel)).unwrap_or_default();
            match release::changelog_section_for(&head, &entry.name, &entry.to) {
                Some(section) => {
                    release::prepend_changelog(&path, &section)?;
                    written.insert(rel);
                }
                None => problems.push(format!(
                    "{rel}: has no section for {} {}",
                    entry.name, entry.to
                )),
            }
        }
        for consumed in consumed {
            fs::remove_file(&consumed.path)
                .with_context(|| format!("remove {}", consumed.path.display()))?;
            written.insert(relative(&consumed.path, scratch)?);
        }
        let mut moved = Vec::new();
        for entry in &plan {
            let manifest = relative(&entry.path, scratch)?;
            let mut names = BTreeSet::from([entry.name.clone()]);
            names.extend(declared_name(&entry.path));
            moved.push(Moved {
                manifest,
                names,
                from: entry.from.clone(),
                to: entry.to.clone(),
            });
        }
        for (path, before) in &before {
            let manifest = relative(path, scratch)?;
            for (name, to) in after.get(path).into_iter().flatten() {
                if let Some(from) = before.get(name)
                    && from != to
                {
                    moved.push(Moved {
                        manifest: manifest.clone(),
                        names: BTreeSet::from([name.clone()]),
                        from: from.clone(),
                        to: to.clone(),
                    });
                }
            }
        }
        Ok(Self {
            written,
            stage: config.prepare.stage.clone(),
            moved,
            problems,
        })
    }
}

/// Each planned `Cargo.toml` in the scratch tree whose working tree copy has
/// a `Cargo.lock`: only there can workspace inheritance move a locked
/// version.
fn cargo_manifests(plan: &[PlanEntry], scratch: &Path, workdir: &Path) -> Result<Vec<PathBuf>> {
    let mut manifests = Vec::new();
    for entry in plan {
        if entry.path.file_name().and_then(|name| name.to_str()) != Some("Cargo.toml") {
            continue;
        }
        let worktree = workdir.join(relative(&entry.path, scratch)?);
        if detect::owner(&worktree).is_some_and(|owner| owner.lockfile.is_some()) {
            manifests.push(entry.path.clone());
        }
    }
    Ok(manifests)
}

/// Every workspace member's version, per planned cargo manifest.
fn cargo_versions(manifests: &[PathBuf]) -> Result<BTreeMap<PathBuf, BTreeMap<String, String>>> {
    manifests
        .iter()
        .map(|manifest| {
            members::versions(manifest)
                .with_context(|| format!("workspace members of {}", manifest.display()))
                .map(|versions| (manifest.clone(), versions))
        })
        .collect()
}

/// The package name a manifest declares, when its format has one verctl
/// reads: `package.name` in `Cargo.toml`, `project.name` in
/// `pyproject.toml`, and `name` in `package.json`.
fn declared_name(manifest: &Path) -> Option<String> {
    let text = fs::read_to_string(manifest).ok()?;
    match manifest.file_name()?.to_str()? {
        "Cargo.toml" => toml_name(&text, "package"),
        "pyproject.toml" => toml_name(&text, "project"),
        "package.json" => serde_json::from_str::<Value>(&text)
            .ok()?
            .get("name")?
            .as_str()
            .map(str::to_owned),
        _ => None,
    }
}

fn toml_name(text: &str, table: &str) -> Option<String> {
    let document = text.parse::<DocumentMut>().ok()?;
    document
        .get(table)?
        .get("name")?
        .as_str()
        .map(str::to_owned)
}

/// The served files, relative to the working tree. Each one that differs
/// from its render is added to `problems`. The render uses the versions on
/// HEAD.
fn served(
    root: &Path,
    head: &Config,
    workdir: &Path,
    problems: &mut Vec<String>,
) -> Result<BTreeSet<String>> {
    if !templates::any(root, &head.templates)? {
        return Ok(BTreeSet::new());
    }
    let versions = release::served_versions(root, head, &[]);
    for drift in templates::drift(root, &head.templates, &head.pins, &versions)? {
        problems.push(format!(
            "{}: differs from what {} renders",
            drift.target.display(),
            drift.template.display()
        ));
    }
    templates::plan(root, &head.templates, &head.pins, &versions)?
        .iter()
        .map(|path| relative(path, workdir))
        .collect()
}

/// Every lockfile change no planned workspace version explains.
fn lock_changes(base: &MergeBase, moved: &[Moved], locks: &[(&str, Kind)]) -> Result<Vec<String>> {
    let mut problems = Vec::new();
    for &(path, kind) in locks {
        let format = match kind {
            Kind::Parsed(format) => format,
            Kind::Unchanged => {
                problems.push(format!(
                    "{path}: records no workspace version, so the version step leaves it unchanged"
                ));
                continue;
            }
            Kind::Unreadable => {
                problems.push(format!(
                    "{path}: verctl does not read this lockfile, so a Version PR may not change it"
                ));
                continue;
            }
        };
        let (Some(before), Some(after)) = (base.base_bytes(path)?, base.worktree_bytes(path)?)
        else {
            problems.push(format!(
                "{path}: the version step neither adds nor deletes a lockfile"
            ));
            continue;
        };
        let (before, after) = match (parse(format, &before), parse(format, &after)) {
            (Ok(before), Ok(after)) => (before, after),
            (Err(error), _) | (_, Err(error)) => {
                problems.push(format!("{path}: {error:#}"));
                continue;
            }
        };
        let lock_dir = Path::new(path).parent().unwrap_or_else(|| Path::new(""));
        for change in before.compare(&after) {
            if !explained(&change, format, lock_dir, moved) {
                problems.push(format!("{path}: {change}"));
            }
        }
    }
    Ok(problems)
}

fn explained(change: &Change, format: Format, lock_dir: &Path, moved: &[Moved]) -> bool {
    let Change::Member {
        label,
        name,
        from,
        to,
    } = change
    else {
        return false;
    };
    moved.iter().any(|member| {
        from.as_deref() == Some(member.from.as_str())
            && to.as_deref() == Some(member.to.as_str())
            && member.records(format, lock_dir, name, label)
    })
}

/// Each lockfile that records a moved workspace version at another version:
/// its owner did not run after the bump.
fn stale_locks(base: &MergeBase, moved: &[Moved]) -> Result<Vec<String>> {
    let workdir = base.workdir();
    let mut problems = Vec::new();
    let mut parsed: BTreeMap<String, Option<(Format, Lockfile)>> = BTreeMap::new();
    for member in moved {
        let Some(lock) =
            detect::owner(&workdir.join(&member.manifest)).and_then(|owner| owner.lockfile)
        else {
            continue;
        };
        let rel = relative(&lock, workdir)?;
        if let Entry::Vacant(slot) = parsed.entry(rel.clone()) {
            let read = match lockfile::kind(&lock) {
                Some(Kind::Parsed(format)) => git::read_optional(&lock)?
                    .and_then(|bytes| parse(format, &bytes).ok())
                    .map(|lockfile| (format, lockfile)),
                _ => None,
            };
            slot.insert(read);
        }
        let Some(Some((format, lockfile))) = parsed.get(&rel) else {
            continue;
        };
        let lock_dir = Path::new(&rel).parent().unwrap_or_else(|| Path::new(""));
        for entry in lockfile.members() {
            if !member.records(*format, lock_dir, &entry.name, &entry.label) {
                continue;
            }
            if *format == Format::Bun && entry.label == "." && entry.version.is_none() {
                continue;
            }
            if entry.version.as_deref() != Some(member.to.as_str()) {
                problems.push(format!(
                    "{rel}: records `{}` at {}, and the version step moves it to {}",
                    entry.label,
                    entry.version.as_deref().unwrap_or("no version"),
                    member.to
                ));
            }
        }
    }
    Ok(problems)
}

fn parse(format: Format, bytes: &[u8]) -> Result<Lockfile> {
    let text = std::str::from_utf8(bytes).context("not UTF-8")?;
    Lockfile::parse(format, text)
}

/// One line naming how a file the version step writes differs from the
/// replay. It lists the keys of a TOML or JSON manifest, or the first
/// differing line.
fn describe(path: &str, expected: Option<&[u8]>, actual: Option<&[u8]>) -> String {
    match (expected, actual) {
        (Some(_), None) => format!("{path}: the version step writes this file, and it is missing"),
        (None, Some(_)) => format!("{path}: the version step removes this file"),
        (Some(expected), Some(actual)) => {
            let at = differences(path, expected, actual);
            if at.is_empty() {
                format!("{path}: differs from what the version step writes")
            } else {
                format!(
                    "{path}: differs from what the version step writes at {}",
                    at.join(", ")
                )
            }
        }
        (None, None) => format!("{path}: differs from what the version step writes"),
    }
}

/// The keys of a TOML or JSON file whose values differ, or the first line
/// that differs in any other text.
fn differences(path: &str, expected: &[u8], actual: &[u8]) -> Vec<String> {
    let (Ok(expected), Ok(actual)) = (std::str::from_utf8(expected), std::str::from_utf8(actual))
    else {
        return Vec::new();
    };
    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);
    let flat = |text: &str| -> Option<BTreeMap<String, String>> {
        let mut out = BTreeMap::new();
        match extension.as_deref() {
            Some("toml") => {
                let document = text.parse::<DocumentMut>().ok()?;
                flatten_toml("", document.as_item(), &mut out);
            }
            Some("json") => {
                flatten_json("", &serde_json::from_str::<Value>(text).ok()?, &mut out);
            }
            _ => return None,
        }
        Some(out)
    };
    if let (Some(expected), Some(actual)) = (flat(expected), flat(actual)) {
        let keys: BTreeSet<&String> = expected.keys().chain(actual.keys()).collect();
        return keys
            .into_iter()
            .filter(|key| expected.get(*key) != actual.get(*key))
            .take(KEYS_NAMED)
            .map(|key| format!("`{key}`"))
            .collect();
    }
    let mut expected_lines = expected.lines();
    let mut actual_lines = actual.lines();
    let mut number = 1;
    loop {
        match (expected_lines.next(), actual_lines.next()) {
            (None, None) => return Vec::new(),
            (left, right) if left == right => number += 1,
            _ => return vec![format!("line {number}")],
        }
    }
}

fn join_key(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_owned()
    } else {
        format!("{prefix}.{key}")
    }
}

fn flatten_toml(prefix: &str, item: &Item, out: &mut BTreeMap<String, String>) {
    if let Some(table) = item.as_table_like() {
        for (key, value) in table.iter() {
            flatten_toml(&join_key(prefix, key), value, out);
        }
    } else if let Some(array) = item.as_array_of_tables() {
        for (index, table) in array.iter().enumerate() {
            for (key, value) in table {
                flatten_toml(
                    &join_key(&join_key(prefix, &index.to_string()), key),
                    value,
                    out,
                );
            }
        }
    } else if let Some(value) = item.as_value() {
        out.insert(prefix.to_owned(), value.to_string().trim().to_owned());
    }
}

fn flatten_json(prefix: &str, value: &Value, out: &mut BTreeMap<String, String>) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                flatten_json(&join_key(prefix, key), value, out);
            }
        }
        Value::Array(items) => {
            for (index, value) in items.iter().enumerate() {
                flatten_json(&join_key(prefix, &index.to_string()), value, out);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            out.insert(prefix.to_owned(), value.to_string());
        }
    }
}

/// `path` relative to `base`, with `/` separators and no `.` components.
pub(crate) fn relative(path: &Path, base: &Path) -> Result<String> {
    let rel = path
        .strip_prefix(base)
        .with_context(|| format!("{} is outside {}", path.display(), base.display()))?;
    let parts: Vec<String> = rel
        .components()
        .filter(|part| !matches!(part, Component::CurDir))
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    Ok(parts.join("/"))
}

/// A command-line path inside the working tree, relative to it. A path that
/// does not exist yet, such as an empty fragment directory, is placed through
/// its parent.
pub(crate) fn inside(workdir: &Path, path: &Path) -> Result<PathBuf> {
    let absolute = if let Ok(absolute) = path.canonicalize() {
        absolute
    } else {
        let name = path.file_name().context("a path with no file name")?;
        let parent = match path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => Path::new("."),
        };
        parent
            .canonicalize()
            .with_context(|| parent.display().to_string())?
            .join(name)
    };
    Ok(PathBuf::from(relative(&absolute, workdir)?))
}

/// A directory under the system temporary directory, removed on drop.
pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub(crate) fn new() -> Result<Self> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default();
        let dir =
            std::env::temp_dir().join(format!("verctl-version-pr-{}-{stamp}", std::process::id()));
        fs::create_dir(&dir).with_context(|| format!("create {}", dir.display()))?;
        match dir.canonicalize() {
            Ok(canonical) => Ok(Self(canonical)),
            Err(error) => {
                fs::remove_dir_all(&dir).ok();
                Err(error).with_context(|| dir.display().to_string())
            }
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}
