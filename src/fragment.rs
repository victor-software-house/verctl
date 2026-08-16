use anyhow::{Context, Result, bail, ensure};
use serde_yml::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Bump {
    None,
    Patch,
    Minor,
    Major,
}

impl Bump {
    pub fn parse(raw: &str) -> Result<Self> {
        match raw {
            "none" => Ok(Self::None),
            "patch" => Ok(Self::Patch),
            "minor" => Ok(Self::Minor),
            "major" => Ok(Self::Major),
            other => bail!("unknown bump type {other:?} (expected major, minor, patch, or none)"),
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Patch => "patch",
            Self::Minor => "minor",
            Self::Major => "major",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageBump {
    pub name: String,
    pub bump: Bump,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fragment {
    pub path: PathBuf,
    pub packages: Vec<PackageBump>,
    pub summary: String,
}

impl Fragment {
    #[must_use]
    pub fn max_bump(&self) -> Bump {
        self.packages
            .iter()
            .map(|package| package.bump)
            .max()
            .unwrap_or(Bump::None)
    }
}

pub fn parse_str(raw: &str, path: impl Into<PathBuf>) -> Result<Fragment> {
    let path = path.into();
    let normalized = normalize_source(raw);
    let (front, body) =
        split_front_matter(&normalized).with_context(|| path.display().to_string())?;
    let yaml: Value =
        serde_yml::from_str(front).with_context(|| format!("yaml in {}", path.display()))?;
    let mapping = yaml.as_mapping().with_context(|| {
        format!(
            "{} front matter must be a mapping of package → bump",
            path.display()
        )
    })?;
    ensure!(
        !mapping.is_empty(),
        "{} front matter has no packages",
        path.display()
    );
    let mut packages = Vec::new();
    let mut seen = BTreeSet::new();
    for (key, value) in mapping {
        let name = match key {
            Value::String(name) => name.clone(),
            other => bail!(
                "{} package name must be a string, got {other:?}",
                path.display()
            ),
        };
        ensure!(
            seen.insert(name.clone()),
            "{} lists {name} more than once",
            path.display()
        );
        let bump_raw = value.as_str().with_context(|| {
            format!(
                "{} bump for {name} must be a string (major, minor, patch, or none)",
                path.display()
            )
        })?;
        packages.push(PackageBump {
            name,
            bump: Bump::parse(bump_raw)?,
        });
    }
    Ok(Fragment {
        path,
        packages,
        summary: body.trim().to_owned(),
    })
}

pub fn load_dir(dir: &Path) -> Result<Vec<Fragment>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .with_context(|| dir.display().to_string())?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| is_fragment_path(path))
        .collect();
    paths.sort();
    let mut fragments = Vec::new();
    for path in paths {
        let raw = fs::read_to_string(&path).with_context(|| path.display().to_string())?;
        fragments.push(parse_str(&raw, path)?);
    }
    Ok(fragments)
}

fn is_fragment_path(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    path.extension().is_some_and(|ext| ext == "md")
        && name != "README.md"
        && !name.eq_ignore_ascii_case("config.md")
}

fn normalize_source(raw: &str) -> String {
    raw.strip_prefix('\u{feff}')
        .unwrap_or(raw)
        .replace("\r\n", "\n")
        .replace('\r', "\n")
}

fn split_front_matter(raw: &str) -> Result<(&str, &str)> {
    let rest = raw
        .strip_prefix("---")
        .context("changeset must start with ---")?;
    let rest = rest.strip_prefix('\n').unwrap_or(rest);
    let (front, body) = rest
        .split_once("\n---")
        .context("changeset is missing the closing --- fence")?;
    let body = body.strip_prefix('\n').unwrap_or(body);
    Ok((front, body))
}
