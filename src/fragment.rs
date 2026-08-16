use anyhow::{Context, Result, bail, ensure};
use gray_matter::Matter;
use gray_matter::engine::YAML;
use serde::Deserialize;
use std::collections::BTreeMap;
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

#[derive(Debug, Deserialize)]
struct FrontMatter(BTreeMap<String, String>);

pub fn parse_str(raw: &str, path: impl Into<PathBuf>) -> Result<Fragment> {
    let path = path.into();
    let normalized = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let mut matter = Matter::<YAML>::new();
    // gray_matter treats --- as an excerpt closer unless we disable it.
    matter.excerpt_delimiter = Some("\u{0000}".into());
    let parsed = matter
        .parse::<FrontMatter>(normalized)
        .with_context(|| format!("front matter in {}", path.display()))?;
    let Some(FrontMatter(mapping)) = parsed.data else {
        bail!("{} changeset must start with ---", path.display());
    };
    ensure!(
        !mapping.is_empty(),
        "{} front matter has no packages",
        path.display()
    );
    let mut packages = Vec::new();
    for (name, bump_raw) in mapping {
        packages.push(PackageBump {
            name,
            bump: Bump::parse(&bump_raw)?,
        });
    }
    Ok(Fragment {
        path,
        packages,
        summary: parsed.content.trim().to_owned(),
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
