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
            other => {
                bail!("unknown bump type {other:?} (expected major, minor, patch, patch!, or none)")
            }
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
    /// Bump the fragment declares.
    pub bump: Bump,
    /// Whether the bump is `patch!`: a patch that breaks the package's API on
    /// purpose. While the fragment is pending, `semver` skips the package, and
    /// the changelog lists the entry under Breaking. Only a patch carries it,
    /// because a minor or major already allows a break.
    pub breaking: bool,
}

impl PackageBump {
    /// The suffix that marks a breaking patch, as `!` marks a breaking change
    /// in a Conventional Commits type ([Conventional Commits 1.0.0]).
    ///
    /// [Conventional Commits 1.0.0]: https://www.conventionalcommits.org/en/v1.0.0/
    pub const BREAKING: char = '!';

    /// Read `name: raw`, where `raw` is a [`Bump`] or `patch!`.
    ///
    /// # Errors
    ///
    /// Returns an error when `raw` is no bump, or when `!` follows anything
    /// but `patch`.
    pub fn parse(name: String, raw: &str) -> Result<Self> {
        let (bare, breaking) = raw
            .strip_suffix(Self::BREAKING)
            .map_or((raw, false), |bare| (bare, true));
        let bump = Bump::parse(bare)?;
        ensure!(
            !breaking || bump == Bump::Patch,
            "{name}: {raw:?}: `!` goes only on patch: a minor or major already allows an API break, and none releases nothing"
        );
        Ok(Self {
            name,
            bump,
            breaking,
        })
    }
}

/// The changelog heading a fragment's entry goes under, from the optional
/// frontmatter key [`Group::KEY`]. Headings follow [Keep a Changelog]'s order.
///
/// [Keep a Changelog]: https://keepachangelog.com/en/1.1.0/
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
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
    /// The heading this entry goes under.
    ///
    /// `None` marks a fragment that names no group. It lands under
    /// [`Group::Changed`] when another entry in the same section names one.
    /// A section where none does stays a flat list.
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
/// Returns an error when:
///
/// - the text does not start with front matter;
/// - the front matter is not a mapping of strings;
/// - its `verctl:group` is unknown;
/// - it names no package;
/// - a bump is not `major`, `minor`, `patch`, or `none`.
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
        packages
            .push(PackageBump::parse(name, &bump_raw).with_context(|| path.display().to_string())?);
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
        .flatten()
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

pub(crate) fn is_fragment_path(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    path.extension().is_some_and(|ext| ext == "md")
        && name != "README.md"
        && !name.eq_ignore_ascii_case("config.md")
}
