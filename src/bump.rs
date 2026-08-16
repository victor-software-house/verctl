use crate::config::ManifestKind;
use crate::fragment::Bump;
use anyhow::{Context, Result, bail};
use semver::Version;
use toml_edit::{DocumentMut, Item, value};

pub fn apply(current: &str, bump: Bump) -> Result<String> {
    if bump == Bump::None {
        return Ok(current.to_owned());
    }
    let mut version = Version::parse(current).with_context(|| format!("semver {current:?}"))?;
    if bump == Bump::Major && version.major == 0 {
        bail!("0.x refuses a major bump (current {current}); use minor or wait for 1.0");
    }
    match bump {
        Bump::None => {}
        Bump::Patch => version.patch += 1,
        Bump::Minor => {
            version.minor += 1;
            version.patch = 0;
        }
        Bump::Major => {
            version.major += 1;
            version.minor = 0;
            version.patch = 0;
        }
    }
    Ok(version.to_string())
}

pub fn read_version(kind: ManifestKind, raw: &str) -> Result<String> {
    match kind {
        ManifestKind::Cargo => read_cargo_version(raw),
        ManifestKind::Npm => read_npm_version(raw),
    }
}

pub fn write_version(kind: ManifestKind, raw: &str, new_version: &str) -> Result<String> {
    match kind {
        ManifestKind::Cargo => write_cargo_version(raw, new_version),
        ManifestKind::Npm => write_npm_version(raw, new_version),
    }
}

fn read_cargo_version(raw: &str) -> Result<String> {
    let doc: DocumentMut = raw.parse().context("parse Cargo.toml")?;
    cargo_version_item(&doc)?
        .as_str()
        .map(ToOwned::to_owned)
        .context("Cargo.toml version is not a string")
}

fn write_cargo_version(raw: &str, new_version: &str) -> Result<String> {
    let mut doc: DocumentMut = raw.parse().context("parse Cargo.toml")?;
    *cargo_version_item_mut(&mut doc)? = value(new_version);
    Ok(doc.to_string())
}

fn cargo_version_item(doc: &DocumentMut) -> Result<&Item> {
    if let Some(item) = doc
        .get("workspace")
        .and_then(|ws| ws.get("package"))
        .and_then(|pkg| pkg.get("version"))
    {
        return Ok(item);
    }
    doc.get("package")
        .and_then(|pkg| pkg.get("version"))
        .context("Cargo.toml has no workspace.package.version or package.version")
}

fn cargo_version_item_mut(doc: &mut DocumentMut) -> Result<&mut Item> {
    if doc
        .get("workspace")
        .and_then(|ws| ws.get("package"))
        .and_then(|pkg| pkg.get("version"))
        .is_some()
    {
        return doc
            .get_mut("workspace")
            .and_then(|ws| ws.get_mut("package"))
            .and_then(|pkg| pkg.get_mut("version"))
            .context("workspace.package.version");
    }
    doc.get_mut("package")
        .and_then(|pkg| pkg.get_mut("version"))
        .context("package.version")
}

fn read_npm_version(raw: &str) -> Result<String> {
    let value: serde_json::Value = serde_json::from_str(raw).context("parse package.json")?;
    value
        .get("version")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .context("package.json has no string version")
}

fn write_npm_version(raw: &str, new_version: &str) -> Result<String> {
    let current = read_npm_version(raw)?;
    if current == new_version {
        return Ok(raw.to_owned());
    }
    let pattern = "\"version\"";
    let Some(key_at) = raw.find(pattern) else {
        bail!("package.json has no \"version\" key");
    };
    let after_key = &raw[key_at + pattern.len()..];
    let colon = after_key
        .find(':')
        .context("package.json version key has no ':'")?;
    let after_colon = &after_key[colon + 1..];
    let quote_rel = after_colon
        .find('"')
        .context("package.json version value is not a string")?;
    let value_start = key_at + pattern.len() + colon + 1 + quote_rel + 1;
    let rest = &raw[value_start..];
    let value_end_rel = rest
        .find('"')
        .context("package.json version string is unterminated")?;
    let value_end = value_start + value_end_rel;
    ensure_slice_is_current(&raw[value_start..value_end], &current)?;
    Ok(format!(
        "{}{}{}",
        &raw[..value_start],
        new_version,
        &raw[value_end..]
    ))
}

fn ensure_slice_is_current(found: &str, current: &str) -> Result<()> {
    if found == current {
        Ok(())
    } else {
        bail!("package.json version text {found:?} != parsed {current:?}")
    }
}
