use anyhow::{Context, Result, bail};
use jsonc_parser::ParseOptions;
use jsonc_parser::cst::{CstInputValue, CstObject, CstRootNode};
use toml_edit::{DocumentMut, Item, value};

use crate::process;

/// The file format a path driver edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// A TOML file, edited with its comments and layout kept.
    Toml,
    /// A JSON file; comments are accepted (JSONC).
    Json,
}

/// How a package's version is read and written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Driver {
    /// The version is a string at a key path in a TOML or JSON file.
    Path {
        /// The file format.
        format: Format,
        /// Dotted key paths tried in order; the first that holds a string wins.
        keys: Vec<String>,
        /// Follow-up command to run after the bump, such as a lockfile refresh.
        after: Option<String>,
    },
    /// An external filter reads and writes the version.
    Command {
        /// Prints the version from the file on stdin.
        read: CommandSpec,
        /// Prints the new file from the file on stdin and `VERCTL_VERSION`.
        write: CommandSpec,
        /// Follow-up command to run after the bump, such as a lockfile refresh.
        after: Option<String>,
    },
}

/// How a command driver's filter is invoked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandSpec {
    /// `mise run <task>` — stdin is the file, stdout is the version or new
    /// file.
    Mise(String),
    /// Execvp. No shell.
    Argv(Vec<String>),
}

impl Driver {
    /// The stock driver for `Cargo.toml`.
    ///
    /// # Errors
    ///
    /// Returns an error only if the built-in definition fails validation,
    /// which a test guards against.
    pub fn cargo() -> Result<Self> {
        crate::config::stock_driver("cargo")
    }

    /// The stock driver for `package.json`.
    ///
    /// # Errors
    ///
    /// Returns an error only if the built-in definition fails validation,
    /// which a test guards against.
    pub fn npm() -> Result<Self> {
        crate::config::stock_driver("npm")
    }

    /// The follow-up command to run after the bump, if any.
    #[must_use]
    pub fn after(&self) -> Option<&str> {
        match self {
            Self::Path { after, .. } | Self::Command { after, .. } => after.as_deref(),
        }
    }

    /// The current version in `raw`.
    ///
    /// # Errors
    ///
    /// For a path driver, returns an error when `raw` is not valid TOML or
    /// JSON, or when none of the configured keys holds a string. For a command
    /// driver, returns an error when the filter cannot be run, times out,
    /// exits unsuccessfully, or prints non-UTF-8 or oversized output.
    pub fn read(&self, raw: &str) -> Result<String> {
        match self {
            Self::Path { format, keys, .. } => match format {
                Format::Toml => read_toml(raw, keys),
                Format::Json => read_json(raw, keys),
            },
            Self::Command { read, .. } => run_filter(read, raw, None),
        }
    }

    /// `raw` with the version replaced by `new_version`.
    ///
    /// # Errors
    ///
    /// Fails on the same conditions as [`Driver::read`]: unparsable input, no
    /// configured key holding a string, or a command filter that fails.
    pub fn write(&self, raw: &str, new_version: &str) -> Result<String> {
        match self {
            Self::Path { format, keys, .. } => match format {
                Format::Toml => write_toml(raw, keys, new_version),
                Format::Json => write_json(raw, keys, new_version),
            },
            Self::Command { write, .. } => run_filter(write, raw, Some(new_version)),
        }
    }
}

fn read_toml(raw: &str, keys: &[String]) -> Result<String> {
    let doc: DocumentMut = raw.parse().context("parse TOML")?;
    for key in keys {
        if let Some(item) = toml_get(&doc, key)
            && let Some(version) = item.as_str()
        {
            return Ok(version.to_owned());
        }
    }
    bail!("none of these TOML keys exist: {}", keys.join(", "))
}

fn write_toml(raw: &str, keys: &[String], new_version: &str) -> Result<String> {
    let mut doc: DocumentMut = raw.parse().context("parse TOML")?;
    for key in keys {
        if toml_get(&doc, key).and_then(Item::as_str).is_some() {
            let item = toml_get_mut(&mut doc, key).context(key.clone())?;
            let decor = item.as_value().map(|old| old.decor().clone());
            *item = value(new_version);
            if let (Some(decor), Some(new)) = (decor, item.as_value_mut()) {
                *new.decor_mut() = decor;
            }
            return Ok(doc.to_string());
        }
    }
    bail!("none of these TOML keys exist: {}", keys.join(", "))
}

fn toml_get<'a>(doc: &'a DocumentMut, path: &str) -> Option<&'a Item> {
    let mut item: Option<&Item> = None;
    for (index, part) in path.split('.').enumerate() {
        item = if index == 0 {
            doc.get(part)
        } else {
            item.and_then(|item| item.get(part))
        };
    }
    item
}

fn toml_get_mut<'a>(doc: &'a mut DocumentMut, path: &str) -> Option<&'a mut Item> {
    let mut parts = path.split('.');
    let first = parts.next()?;
    let mut item = doc.get_mut(first)?;
    for part in parts {
        item = item.get_mut(part)?;
    }
    Some(item)
}

fn read_json(raw: &str, keys: &[String]) -> Result<String> {
    let root = parse_json(raw)?;
    for key in keys {
        if let Some(version) = json_string_at(&root, key) {
            return Ok(version);
        }
    }
    bail!("none of these JSON keys exist: {}", keys.join(", "))
}

fn write_json(raw: &str, keys: &[String], new_version: &str) -> Result<String> {
    let root = parse_json(raw)?;
    for key in keys {
        if json_set_string(&root, key, new_version) {
            return Ok(root.to_string());
        }
    }
    bail!("none of these JSON keys exist: {}", keys.join(", "))
}

fn parse_json(raw: &str) -> Result<CstRootNode> {
    CstRootNode::parse(raw, &ParseOptions::default()).context("parse JSON")
}

fn json_object_at(object: &CstObject, path: &str) -> Option<CstObject> {
    let mut current = object.clone();
    for part in path.split('.') {
        current = current.object_value(part)?;
    }
    Some(current)
}

fn json_string_at(root: &CstRootNode, path: &str) -> Option<String> {
    let object = root.object_value()?;
    let (parent_path, field) = split_json_path(path);
    let parent = match parent_path {
        Some(parent) => json_object_at(&object, parent)?,
        None => object,
    };
    parent
        .get(field)?
        .value()?
        .as_string_lit()?
        .decoded_value()
        .ok()
}

fn json_set_string(root: &CstRootNode, path: &str, new_version: &str) -> bool {
    let Some(object) = root.object_value() else {
        return false;
    };
    let (parent_path, field) = split_json_path(path);
    let parent = match parent_path {
        Some(parent) => match json_object_at(&object, parent) {
            Some(parent) => parent,
            None => return false,
        },
        None => object,
    };
    let Some(prop) = parent.get(field) else {
        return false;
    };
    if prop
        .value()
        .and_then(|value| value.as_string_lit())
        .is_none()
    {
        return false;
    }
    prop.set_value(CstInputValue::String(new_version.to_owned()));
    true
}

fn split_json_path(path: &str) -> (Option<&str>, &str) {
    match path.rsplit_once('.') {
        Some((parent, field)) => (Some(parent), field),
        None => (None, path),
    }
}

fn run_filter(spec: &CommandSpec, stdin: &str, new_version: Option<&str>) -> Result<String> {
    let argv = match spec {
        CommandSpec::Mise(task) => vec!["mise".into(), "run".into(), task.clone()],
        CommandSpec::Argv(argv) => argv.clone(),
    };
    let env = new_version.map_or_else(Vec::new, |version| vec![("VERCTL_VERSION", version)]);
    let stdout = process::filter(&argv, stdin, &env)?;
    if new_version.is_some() {
        Ok(stdout)
    } else {
        Ok(stdout.trim().to_owned())
    }
}
