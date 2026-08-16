use anyhow::{Context, Result, bail, ensure};
use serde_json::Value as Json;
use std::io::Write;
use std::process::{Command, Stdio};
use toml_edit::{DocumentMut, Item, value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Toml,
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Driver {
    Path {
        format: Format,
        keys: Vec<String>,
        after: Option<String>,
    },
    Shell {
        read: String,
        write: String,
        after: Option<String>,
    },
}

impl Driver {
    #[must_use]
    pub fn cargo() -> Self {
        Self::Path {
            format: Format::Toml,
            keys: vec!["workspace.package.version".into(), "package.version".into()],
            after: Some("cargo generate-lockfile".into()),
        }
    }

    #[must_use]
    pub fn npm() -> Self {
        Self::Path {
            format: Format::Json,
            keys: vec!["version".into()],
            after: Some("bun install".into()),
        }
    }

    #[must_use]
    pub fn after(&self) -> Option<&str> {
        match self {
            Self::Path { after, .. } | Self::Shell { after, .. } => after.as_deref(),
        }
    }

    pub fn read(&self, raw: &str) -> Result<String> {
        match self {
            Self::Path { format, keys, .. } => match format {
                Format::Toml => read_toml(raw, keys),
                Format::Json => read_json(raw, keys),
            },
            Self::Shell { read, .. } => run_filter(read, raw, None),
        }
    }

    pub fn write(&self, raw: &str, new_version: &str) -> Result<String> {
        match self {
            Self::Path { format, keys, .. } => match format {
                Format::Toml => write_toml(raw, keys, new_version),
                Format::Json => write_json(raw, keys, new_version),
            },
            Self::Shell { write, .. } => run_filter(write, raw, Some(new_version)),
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
            *toml_get_mut(&mut doc, key).context(key.clone())? = value(new_version);
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
    let value: Json = serde_json::from_str(raw).context("parse JSON")?;
    for key in keys {
        if let Some(version) = json_get(&value, key).and_then(Json::as_str) {
            return Ok(version.to_owned());
        }
    }
    bail!("none of these JSON keys exist: {}", keys.join(", "))
}

fn write_json(raw: &str, keys: &[String], new_version: &str) -> Result<String> {
    let value: Json = serde_json::from_str(raw).context("parse JSON")?;
    let current = keys
        .iter()
        .find_map(|key| json_get(&value, key).and_then(Json::as_str))
        .context("JSON version key missing")?
        .to_owned();
    if current == new_version {
        return Ok(raw.to_owned());
    }
    let field = keys
        .iter()
        .find(|key| json_get(&value, key).is_some())
        .context("JSON key vanished")?;
    let last = field.rsplit('.').next().context("empty JSON key")?;
    replace_json_string_field(raw, last, &current, new_version)
}

fn json_get<'a>(value: &'a Json, path: &str) -> Option<&'a Json> {
    let mut cur = value;
    for part in path.split('.') {
        cur = cur.get(part)?;
    }
    Some(cur)
}

fn replace_json_string_field(
    raw: &str,
    field: &str,
    current: &str,
    new_version: &str,
) -> Result<String> {
    let pattern = format!("\"{field}\"");
    let Some(key_at) = raw.find(&pattern) else {
        bail!("JSON has no {field:?} key");
    };
    let after_key = &raw[key_at + pattern.len()..];
    let colon = after_key.find(':').context("JSON key has no ':'")?;
    let after_colon = &after_key[colon + 1..];
    let quote_rel = after_colon
        .find('"')
        .context("JSON value is not a string")?;
    let value_start = key_at + pattern.len() + colon + 1 + quote_rel + 1;
    let rest = &raw[value_start..];
    let value_end_rel = rest.find('"').context("JSON string is unterminated")?;
    let value_end = value_start + value_end_rel;
    ensure!(
        &raw[value_start..value_end] == current,
        "JSON text {:?} != parsed {current:?}",
        &raw[value_start..value_end]
    );
    Ok(format!(
        "{}{}{}",
        &raw[..value_start],
        new_version,
        &raw[value_end..]
    ))
}

fn run_filter(script: &str, stdin: &str, new_version: Option<&str>) -> Result<String> {
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(version) = new_version {
        command.env("VERCTL_VERSION", version);
    }
    let mut child = command.spawn().context("spawn driver command")?;
    child
        .stdin
        .as_mut()
        .context("driver stdin")?
        .write_all(stdin.as_bytes())
        .context("write driver stdin")?;
    let output = child.wait_with_output().context("driver command")?;
    if !output.status.success() {
        bail!(
            "driver command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let stdout = String::from_utf8(output.stdout).context("driver stdout is not UTF-8")?;
    if new_version.is_some() {
        Ok(stdout)
    } else {
        Ok(stdout.trim().to_owned())
    }
}
