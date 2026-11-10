//! The lockfiles a Version PR may change, read to prove it moved only
//! workspace versions.
//!
//! verctl never writes a lockfile: each moves through its owner
//! ([`crate::detect::owner`]). `check` parses the merge-base and HEAD copies
//! and compares them entry by entry with [`Lockfile::compare`].
//!
//! A registry entry and a workspace member can share a name, so they differ
//! by source:
//!
//! - `Cargo.lock`: a member has no `source`.
//! - `bun.lock`: a member sits under a `workspaces` key.
//! - `package-lock.json`: a member is a path outside `node_modules/`.
//! - `uv.lock`: a member has an `editable` or `virtual` source.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value};

/// How `check` treats a lockfile on a Version PR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Kind {
    /// verctl parses it, and only workspace versions may move.
    Parsed(Format),
    /// It records no workspace version, so a Version PR leaves it unchanged:
    /// `pnpm-lock.yaml`, `poetry.lock`, and `pdm.lock`.
    Unchanged,
    /// verctl does not read it, so a Version PR that changes it fails:
    /// `yarn.lock` and the binary `bun.lockb`.
    Unreadable,
}

/// A lockfile format verctl parses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Format {
    /// `Cargo.lock`.
    Cargo,
    /// The text `bun.lock`, which is JSON with trailing commas.
    Bun,
    /// `package-lock.json` or `npm-shrinkwrap.json`, version 2 or 3.
    Npm,
    /// `uv.lock`.
    Uv,
}

/// How `check` treats the file at `path`, or `None` when it is no lockfile.
#[must_use]
pub fn kind(path: &Path) -> Option<Kind> {
    match path.file_name()?.to_str()? {
        "Cargo.lock" => Some(Kind::Parsed(Format::Cargo)),
        "bun.lock" => Some(Kind::Parsed(Format::Bun)),
        "package-lock.json" | "npm-shrinkwrap.json" => Some(Kind::Parsed(Format::Npm)),
        "uv.lock" => Some(Kind::Parsed(Format::Uv)),
        "pnpm-lock.yaml" | "poetry.lock" | "pdm.lock" => Some(Kind::Unchanged),
        "yarn.lock" | "bun.lockb" => Some(Kind::Unreadable),
        _ => None,
    }
}

/// One locked package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// What an error names: the package name, or the path of a JavaScript
    /// workspace member (`.` for the root) or of a `node_modules` entry.
    pub label: String,
    /// The package name, as its manifest writes it.
    pub name: String,
    /// The locked version, when the entry records one.
    pub version: Option<String>,
    /// Whether the entry is a workspace member.
    pub member: bool,
    /// Every other field, written with sorted keys. A reference to a member
    /// carries no version, so a member's bump leaves its dependents' entries
    /// unchanged.
    rest: String,
}

/// A parsed lockfile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lockfile {
    /// Every top-level field but the entries, written with sorted keys.
    header: String,
    /// Entries by a key unique within the file.
    entries: BTreeMap<String, Entry>,
}

/// One difference between two copies of a lockfile.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Change {
    /// A top-level field changed, such as the format version.
    Header,
    /// An entry exists only in the newer copy.
    Added(String),
    /// An entry exists only in the older copy.
    Removed(String),
    /// An entry changed in a field other than a member's version, or a
    /// registry entry changed its version.
    Changed(String),
    /// A workspace member's version moved and nothing else in its entry did.
    Member {
        /// The entry's label.
        label: String,
        /// The member's package name.
        name: String,
        /// The version in the older copy.
        from: Option<String>,
        /// The version in the newer copy.
        to: Option<String>,
    },
}

impl Change {
    /// Whether the change is a member's version alone, the only change a
    /// lockfile owner makes for a bump.
    #[must_use]
    pub fn is_member_version(&self) -> bool {
        matches!(self, Self::Member { .. })
    }
}

impl fmt::Display for Change {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Header => write!(f, "its header changed"),
            Self::Added(label) => write!(f, "`{label}` was added"),
            Self::Removed(label) => write!(f, "`{label}` was removed"),
            Self::Changed(label) => write!(f, "`{label}` changed"),
            Self::Member {
                label, from, to, ..
            } => write!(
                f,
                "`{label}` moved from {} to {}",
                from.as_deref().unwrap_or("no version"),
                to.as_deref().unwrap_or("no version")
            ),
        }
    }
}

impl Lockfile {
    /// Parse `text` as `format`.
    ///
    /// # Errors
    ///
    /// Returns an error when the text does not parse, or an entry lacks the
    /// fields its format requires, such as a `Cargo.lock` package without a
    /// name.
    pub fn parse(format: Format, text: &str) -> Result<Self> {
        match format {
            Format::Cargo => parse_cargo(text),
            Format::Bun => parse_bun(text),
            Format::Npm => parse_npm(text),
            Format::Uv => parse_uv(text),
        }
    }

    /// Every entry, in key order.
    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.values()
    }

    /// The workspace members.
    pub fn members(&self) -> impl Iterator<Item = &Entry> {
        self.entries().filter(|entry| entry.member)
    }

    /// What changed from `self` to `newer`, in key order. An entry whose key
    /// carries its version, a registry package's, shows a version move as
    /// one [`Change::Changed`] instead of a removal and an addition.
    #[must_use]
    pub fn compare(&self, newer: &Self) -> Vec<Change> {
        let mut changes = Vec::new();
        if self.header != newer.header {
            changes.push(Change::Header);
        }
        let mut removed = BTreeSet::new();
        let mut added = BTreeSet::new();
        for (key, old) in &self.entries {
            match newer.entries.get(key) {
                None => {
                    removed.insert(old.label.clone());
                }
                Some(new) if new == old => {}
                Some(new)
                    if new.member && old.member && new.name == old.name && new.rest == old.rest =>
                {
                    changes.push(Change::Member {
                        label: new.label.clone(),
                        name: new.name.clone(),
                        from: old.version.clone(),
                        to: new.version.clone(),
                    });
                }
                Some(new) => changes.push(Change::Changed(new.label.clone())),
            }
        }
        for (key, new) in &newer.entries {
            if !self.entries.contains_key(key) {
                added.insert(new.label.clone());
            }
        }
        for label in removed.intersection(&added) {
            changes.push(Change::Changed(label.clone()));
        }
        changes.extend(removed.difference(&added).cloned().map(Change::Removed));
        changes.extend(added.difference(&removed).cloned().map(Change::Added));
        changes
    }

    fn insert(&mut self, key: String, entry: Entry) -> Result<()> {
        let label = entry.label.clone();
        if self.entries.insert(key, entry).is_some() {
            bail!("`{label}` is locked twice");
        }
        Ok(())
    }
}

/// `Cargo.lock`: `[[package]]` tables. A member has no `source`, and neither
/// has a path dependency outside the workspace, so both read as members here.
/// The content check explains a member's move only by a version the plan
/// moves, so a path dependency that moves still fails there. A
/// dependency reference is `name`, `name version`, or `name version
/// (source)`; a reference to a member drops its version.
fn parse_cargo(text: &str) -> Result<Lockfile> {
    let doc: toml_edit::DocumentMut = text.parse().context("parse Cargo.lock")?;
    let mut top = to_json_table(doc.as_table());
    let packages = take_array(&mut top, "package");
    let members: BTreeSet<String> = packages
        .iter()
        .filter(|package| package.get("source").is_none())
        .filter_map(|package| package.get("name").and_then(Value::as_str))
        .map(str::to_owned)
        .collect();
    let mut lock = Lockfile {
        header: canonical(&Value::Object(top)),
        entries: BTreeMap::new(),
    };
    for package in packages {
        let Value::Object(mut fields) = package else {
            bail!("Cargo.lock: a [[package]] is not a table");
        };
        let name = take_string(&mut fields, "name").context("Cargo.lock: a package has no name")?;
        let version = take_string(&mut fields, "version");
        let member = !fields.contains_key("source");
        if let Some(Value::Array(dependencies)) = fields.get_mut("dependencies") {
            for dependency in dependencies.iter_mut() {
                if let Value::String(reference) = dependency
                    && !reference.contains('(')
                    && let Some(target) = reference.split_whitespace().next()
                    && members.contains(target)
                {
                    *reference = target.to_owned();
                }
            }
        }
        let key = if member {
            format!("member {name}")
        } else {
            format!(
                "{name} {} {}",
                version.as_deref().unwrap_or_default(),
                fields.get("source").map(canonical).unwrap_or_default()
            )
        };
        let entry = Entry {
            label: name.clone(),
            name,
            version,
            member,
            rest: canonical(&Value::Object(fields)),
        };
        lock.insert(key, entry)?;
    }
    Ok(lock)
}

/// `uv.lock`: `[[package]]` tables. A member's `source` is `editable` or
/// `virtual`. A dependency is an inline table; a reference to a member drops
/// its `version`.
fn parse_uv(text: &str) -> Result<Lockfile> {
    let doc: toml_edit::DocumentMut = text.parse().context("parse uv.lock")?;
    let mut top = to_json_table(doc.as_table());
    let packages = take_array(&mut top, "package");
    let is_member = |package: &Value| {
        package.get("source").is_some_and(|source| {
            source.get("editable").is_some() || source.get("virtual").is_some()
        })
    };
    let members: BTreeSet<String> = packages
        .iter()
        .filter(|package| is_member(package))
        .filter_map(|package| package.get("name").and_then(Value::as_str))
        .map(str::to_owned)
        .collect();
    let mut lock = Lockfile {
        header: canonical(&Value::Object(top)),
        entries: BTreeMap::new(),
    };
    for package in packages {
        let member = is_member(&package);
        let Value::Object(mut fields) = package else {
            bail!("uv.lock: a [[package]] is not a table");
        };
        let name = take_string(&mut fields, "name").context("uv.lock: a package has no name")?;
        let version = take_string(&mut fields, "version");
        for list in ["dependencies", "dev-dependencies", "optional-dependencies"] {
            if let Some(value) = fields.get_mut(list) {
                drop_member_versions(value, &members);
            }
        }
        let key = if member {
            format!("member {name}")
        } else {
            format!(
                "{name} {} {}",
                version.as_deref().unwrap_or_default(),
                fields.get("source").map(canonical).unwrap_or_default()
            )
        };
        let entry = Entry {
            label: name.clone(),
            name,
            version,
            member,
            rest: canonical(&Value::Object(fields)),
        };
        lock.insert(key, entry)?;
    }
    Ok(lock)
}

/// Drop `version` from each `{ name = ... }` reference to a member, at any
/// depth: `optional-dependencies` and `dev-dependencies` nest the lists in a
/// table of groups.
fn drop_member_versions(value: &mut Value, members: &BTreeSet<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                drop_member_versions(item, members);
            }
        }
        Value::Object(fields) => {
            let names_member = fields
                .get("name")
                .and_then(Value::as_str)
                .is_some_and(|name| members.contains(name));
            if names_member {
                fields.remove("version");
            } else {
                for nested in fields.values_mut() {
                    drop_member_versions(nested, members);
                }
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

/// `bun.lock`: each `workspaces` entry is a member, keyed by its path; each
/// `packages` entry is a resolved package, whose first element is
/// `name@version` or `name@workspace:path`.
fn parse_bun(text: &str) -> Result<Lockfile> {
    let value = jsonc_parser::parse_to_value(text, &jsonc_parser::ParseOptions::default())
        .context("parse bun.lock")?
        .context("bun.lock is empty")?;
    let Value::Object(mut top) = from_jsonc(value) else {
        bail!("bun.lock is not an object");
    };
    let workspaces = take_object(&mut top, "workspaces");
    let packages = take_object(&mut top, "packages");
    let mut lock = Lockfile {
        header: canonical(&Value::Object(top)),
        entries: BTreeMap::new(),
    };
    for (path, workspace) in workspaces {
        let Value::Object(mut fields) = workspace else {
            bail!("bun.lock: workspace `{path}` is not an object");
        };
        let name = take_string(&mut fields, "name").unwrap_or_default();
        let version = take_string(&mut fields, "version");
        let label = if path.is_empty() {
            ".".to_owned()
        } else {
            path.clone()
        };
        let entry = Entry {
            label,
            name,
            version,
            member: true,
            rest: canonical(&Value::Object(fields)),
        };
        lock.insert(format!("workspace {path}"), entry)?;
    }
    for (key, package) in packages {
        let ident = package
            .get(0)
            .and_then(Value::as_str)
            .with_context(|| format!("bun.lock: package `{key}` has no name@version"))?;
        let (name, resolved) = ident
            .rsplit_once('@')
            .filter(|(name, _)| !name.is_empty())
            .with_context(|| format!("bun.lock: package `{key}` is `{ident}`, not name@version"))?;
        let version = (!resolved.starts_with("workspace:")).then(|| resolved.to_owned());
        let entry = Entry {
            label: key.clone(),
            name: name.to_owned(),
            version,
            member: false,
            rest: canonical(&package),
        };
        lock.insert(format!("package {key}"), entry)?;
    }
    Ok(lock)
}

/// `package-lock.json`, lockfile version 2 or 3: `packages` keyed by path.
/// `""` is the root, and a path outside `node_modules/` is a workspace member.
/// A `node_modules/` path is an installed package or a link to a member, and a
/// link records no version. The top-level `version` is the root's.
fn parse_npm(text: &str) -> Result<Lockfile> {
    let Value::Object(mut top) =
        serde_json::from_str::<Value>(text).context("parse package-lock.json")?
    else {
        bail!("package-lock.json is not an object");
    };
    let packages = take_object(&mut top, "packages");
    let root_version = take_string(&mut top, "version");
    let mut lock = Lockfile {
        header: canonical(&Value::Object(top)),
        entries: BTreeMap::new(),
    };
    for (path, package) in packages {
        let Value::Object(mut fields) = package else {
            bail!("package-lock.json: `{path}` is not an object");
        };
        let member = path.is_empty() || !path.contains("node_modules/");
        let version = take_string(&mut fields, "version");
        if path.is_empty() && version != root_version {
            bail!("package-lock.json: the top-level version differs from the root package's");
        }
        let name = fields
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| {
                path.rsplit_once("node_modules/")
                    .map(|(_, name)| name.to_owned())
            })
            .unwrap_or_else(|| path.rsplit('/').next().unwrap_or_default().to_owned());
        let label = if path.is_empty() {
            ".".to_owned()
        } else {
            path.clone()
        };
        let rest = if member {
            canonical(&Value::Object(fields))
        } else {
            let mut whole = fields;
            if let Some(version) = &version {
                whole.insert("version".to_owned(), Value::String(version.clone()));
            }
            canonical(&Value::Object(whole))
        };
        let entry = Entry {
            label,
            name,
            version,
            member,
            rest,
        };
        lock.insert(format!("path {path}"), entry)?;
    }
    Ok(lock)
}

fn take_string(fields: &mut Map<String, Value>, key: &str) -> Option<String> {
    match fields.remove(key) {
        Some(Value::String(text)) => Some(text),
        Some(other) => {
            fields.insert(key.to_owned(), other);
            None
        }
        None => None,
    }
}

fn take_array(fields: &mut Map<String, Value>, key: &str) -> Vec<Value> {
    match fields.remove(key) {
        Some(Value::Array(items)) => items,
        Some(other) => {
            fields.insert(key.to_owned(), other);
            Vec::new()
        }
        None => Vec::new(),
    }
}

fn take_object(fields: &mut Map<String, Value>, key: &str) -> Map<String, Value> {
    match fields.remove(key) {
        Some(Value::Object(object)) => object,
        Some(other) => {
            fields.insert(key.to_owned(), other);
            Map::new()
        }
        None => Map::new(),
    }
}

/// `value` as JSON with object keys sorted at every depth, so two copies
/// that differ only in key order or formatting compare equal.
fn canonical(value: &Value) -> String {
    match value {
        Value::Object(fields) => {
            let sorted: BTreeMap<&String, String> = fields
                .iter()
                .map(|(key, value)| (key, canonical(value)))
                .collect();
            let body: Vec<String> = sorted
                .into_iter()
                .map(|(key, value)| format!("{}:{value}", Value::String(key.clone())))
                .collect();
            format!("{{{}}}", body.join(","))
        }
        Value::Array(items) => {
            let body: Vec<String> = items.iter().map(canonical).collect();
            format!("[{}]", body.join(","))
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => value.to_string(),
    }
}

fn from_jsonc(value: jsonc_parser::JsonValue<'_>) -> Value {
    use jsonc_parser::JsonValue;
    match value {
        JsonValue::String(text) => Value::String(text.into_owned()),
        JsonValue::Number(raw) => {
            serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_owned()))
        }
        JsonValue::Boolean(flag) => Value::Bool(flag),
        JsonValue::Object(object) => Value::Object(
            object
                .into_iter()
                .map(|(key, value)| (key.into_owned(), from_jsonc(value)))
                .collect(),
        ),
        JsonValue::Array(items) => Value::Array(items.into_iter().map(from_jsonc).collect()),
        JsonValue::Null => Value::Null,
    }
}

fn to_json_table(table: &toml_edit::Table) -> Map<String, Value> {
    table
        .iter()
        .filter_map(|(key, item)| to_json_item(item).map(|value| (key.to_owned(), value)))
        .collect()
}

fn to_json_item(item: &toml_edit::Item) -> Option<Value> {
    match item {
        toml_edit::Item::None => None,
        toml_edit::Item::Value(value) => Some(to_json_value(value)),
        toml_edit::Item::Table(table) => Some(Value::Object(to_json_table(table))),
        toml_edit::Item::ArrayOfTables(tables) => Some(Value::Array(
            tables
                .iter()
                .map(|table| Value::Object(to_json_table(table)))
                .collect(),
        )),
    }
}

fn to_json_value(value: &toml_edit::Value) -> Value {
    match value {
        toml_edit::Value::String(text) => Value::String(text.value().clone()),
        toml_edit::Value::Integer(number) => Value::from(*number.value()),
        toml_edit::Value::Float(number) => Value::from(*number.value()),
        toml_edit::Value::Boolean(flag) => Value::Bool(*flag.value()),
        toml_edit::Value::Datetime(when) => Value::String(when.value().to_string()),
        toml_edit::Value::Array(items) => Value::Array(items.iter().map(to_json_value).collect()),
        toml_edit::Value::InlineTable(table) => Value::Object(
            table
                .iter()
                .map(|(key, value)| (key.to_owned(), to_json_value(value)))
                .collect(),
        ),
    }
}
