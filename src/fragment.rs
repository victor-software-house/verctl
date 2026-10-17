use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use ctl_core::input::Input;

/// How far a fragment moves a package's version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Bump {
    /// No version change.
    None,
    /// Patch release.
    Patch,
    /// Minor release.
    Minor,
    /// Major release.
    Major,
}

impl Bump {
    /// Read `major`, `minor`, `patch`, or `none`.
    ///
    /// # Errors
    ///
    /// Returns an error when `raw` is any other string.
    pub fn parse(raw: &str) -> Result<Self> {
        match raw {
            "none" => Ok(Self::None),
            "patch" => Ok(Self::Patch),
            "minor" => Ok(Self::Minor),
            "major" => Ok(Self::Major),
            other => bail!("unknown bump type {other:?} (expected major, minor, patch, or none)"),
        }
    }

    /// The lowercase name `parse` reads.
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

/// A package named in a fragment's frontmatter, with its bump.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageBump {
    /// Package name.
    pub name: String,
    /// Bump the fragment asks for.
    pub bump: Bump,
}

/// The changelog heading a fragment's entry goes under, from the optional
/// frontmatter key [`Group::KEY`]. Headings follow Keep a Changelog's order
/// (<https://keepachangelog.com/en/1.1.0/>).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    /// New functionality.
    Added,
    /// Changed behaviour.
    Changed,
    /// Bug fix.
    Fixed,
}

impl Group {
    /// The frontmatter key. A colon cannot appear in a Cargo or npm package
    /// name, so the key never shadows a package.
    pub const KEY: &'static str = "verctl:group";

    /// Every group, in heading order.
    pub const ALL: [Self; 3] = [Self::Added, Self::Changed, Self::Fixed];

    /// Read `added`, `changed`, or `fixed`.
    ///
    /// # Errors
    ///
    /// Returns an error when `raw` is any other string.
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

    /// The capitalised changelog heading.
    #[must_use]
    pub fn heading(self) -> &'static str {
        match self {
            Self::Added => "Added",
            Self::Changed => "Changed",
            Self::Fixed => "Fixed",
        }
    }
}

/// One Changesets-format file under `.changeset/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fragment {
    /// The fragment file.
    pub path: PathBuf,
    /// Packages the frontmatter bumps.
    pub packages: Vec<PackageBump>,
    /// The body, trimmed. It becomes the changelog entry.
    pub summary: String,
    /// The heading this entry goes under. `None` is a fragment that names no
    /// group: it lands under [`Group::Changed`] when another entry in the same
    /// section names one, and a section where none does stays a flat list.
    pub group: Option<Group>,
}

impl Fragment {
    /// The largest bump among the packages.
    #[must_use]
    pub fn max_bump(&self) -> Bump {
        self.packages
            .iter()
            .map(|package| package.bump)
            .max()
            .unwrap_or(Bump::None)
    }
}

/// Parse a fragment from its raw text. `path` names it in errors.
///
/// # Errors
///
/// Returns an error when the text does not start with front matter, when the
/// front matter is not a mapping of strings, when its `verctl:group` is
/// unknown, when it names no package, or when a bump is not `major`, `minor`,
/// `patch`, or `none`.
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

/// Load every fragment in `dir`, sorted by path. A missing directory yields
/// none.
///
/// # Errors
///
/// Returns an error when `dir` cannot be read, or when a fragment file in it
/// cannot be read or fails [`parse_str`].
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
