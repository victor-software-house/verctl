use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use ctl_core::input::Input;

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

/// The changelog heading a fragment's entry goes under, from the optional
/// frontmatter key [`Group::KEY`]. Headings follow Keep a Changelog's order
/// (<https://keepachangelog.com/en/1.1.0/>).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Added,
    Changed,
    Fixed,
}

impl Group {
    /// The frontmatter key. A colon cannot appear in a Cargo or npm package
    /// name, so the key never shadows a package.
    pub const KEY: &'static str = "verctl:group";

    /// Every group, in heading order.
    pub const ALL: [Self; 3] = [Self::Added, Self::Changed, Self::Fixed];

    pub fn parse(raw: &str) -> Result<Self> {
        match raw {
            "added" => Ok(Self::Added),
            "changed" => Ok(Self::Changed),
            "fixed" => Ok(Self::Fixed),
            other => bail!(
                "unknown {} {other:?} (expected added, changed, or fixed)",
                Self::KEY
            ),
        }
    }

    #[must_use]
    pub fn heading(self) -> &'static str {
        match self {
            Self::Added => "Added",
            Self::Changed => "Changed",
            Self::Fixed => "Fixed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fragment {
    pub path: PathBuf,
    pub packages: Vec<PackageBump>,
    pub summary: String,
    /// The heading this entry goes under. `None` is a fragment that names no
    /// group: it lands under [`Group::Changed`] when another entry in the same
    /// section names one, and a section where none does stays a flat list.
    pub group: Option<Group>,
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
    let input = Input::new(path.display().to_string(), raw);
    let Some((matter, body)) = input.frontmatter() else {
        bail!("{} changeset must start with ---", path.display());
    };
    let mut mapping: BTreeMap<String, String> = if matter.text().trim().is_empty() {
        BTreeMap::new()
    } else {
        matter.parse()?
    };
    let group = mapping
        .remove(Group::KEY)
        .map(|raw| Group::parse(&raw))
        .transpose()
        .with_context(|| path.display().to_string())?;
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
        summary: body.trim().to_owned(),
        group,
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
