use anyhow::{Context, Result, bail, ensure};
use semver::Version;

use crate::config::Prerelease;
use crate::fragment::Bump;

/// Whether `version` is a new package's placeholder: `0.0.0`, with or without
/// a pre-release or build suffix. A new package releases `0.0.1` first, so a
/// placeholder is never a release. A version that is not semver is not one.
#[must_use]
pub fn unreleased(version: &str) -> bool {
    Version::parse(version)
        .is_ok_and(|version| (version.major, version.minor, version.patch) == (0, 0, 0))
}

/// Whether `earlier` is strictly lower than `later`, both as semver. A version
/// that is not semver compares as not lower, so a caller that walks history
/// stops at it rather than guess an order.
#[must_use]
pub fn lower(earlier: &str, later: &str) -> bool {
    matches!(
        (Version::parse(earlier), Version::parse(later)),
        (Ok(earlier), Ok(later)) if earlier < later
    )
}

/// The stable version `bump` moves `current` to: [`apply_with`] for a package
/// that sets no `prerelease`.
///
/// A current prerelease keeps its base when the base already reaches the
/// bump, so `1.2.3-rc.1` with a patch gives `1.2.3`.
///
/// # Errors
///
/// Returns an error as [`apply_with`] does without a `prerelease`.
pub fn apply(current: &str, bump: Bump, first_minor: bool) -> Result<String> {
    apply_with(current, bump, first_minor, None)
}

/// The version `bump` moves `current` to, as a stable release or, when the
/// package sets `prerelease`, as a candidate. [`apply`] is this with no
/// `prerelease`.
///
/// The base is `current` bumped by `bump`, except that a current prerelease
/// keeps its base when the base already reaches the bump: a patch always, a
/// minor when the patch is 0, a major when the minor and patch are 0. That is
/// what node-semver's `inc` does, and Changesets versions through it.
///
/// With `prerelease`, the version is `<base>-<tag>.<count>`. The count is the
/// current count plus 1 when the base and the tag are unchanged, and 1
/// otherwise. Without it, the version is the base. A stable `current` that
/// `bump` leaves alone stays as it is, and so does a candidate of the tag
/// `prerelease` names.
///
/// Two bumps are refused:
///
/// - A `major` on any 0.x package, because 1.0 is a decision rather than a
///   fragment.
/// - A `minor` or `major` on a `0.0.0` package, including one with a
///   pre-release or build suffix, because a new package releases `0.0.1` first.
///
/// The operator records `first_minor` on a package to open it at `0.1.0`.
///
/// # Errors
///
/// Returns an error when `current` is not a semver version, when the bump is
/// one of the two refused above, when `prerelease` is set and `current` has a
/// prerelease that is not `<letters>.<number>`, or when the candidate would
/// sort below `current`, as `0.8.0-beta.1` does below `0.8.0-rc.2`.
pub fn apply_with(
    current: &str,
    bump: Bump,
    first_minor: bool,
    prerelease: Option<&Prerelease>,
) -> Result<String> {
    let version = Version::parse(current).with_context(|| format!("semver {current:?}"))?;
    let counted = prerelease
        .filter(|_| !version.pre.is_empty())
        .map(|tag| Counted::read(&version, tag))
        .transpose()?;
    let settled = counted
        .as_ref()
        .map_or(version.pre.is_empty(), |counted| counted.same_tag);
    if bump == Bump::None && settled {
        return Ok(current.to_owned());
    }
    if unreleased(current) && bump > Bump::Patch && !first_minor {
        bail!(
            "0.0.0 refuses a {} bump: a new package releases 0.0.1 first, from a patch \
             fragment; set `first_minor: true` on the package to open at 0.1.0",
            bump.as_str()
        );
    }
    if bump == Bump::Major && version.major == 0 {
        bail!("0.x refuses a major bump (current {current}); use minor or wait for 1.0");
    }
    let mut next = Version::new(version.major, version.minor, version.patch);
    if version.pre.is_empty() || !reaches(&version, bump) {
        match bump {
            Bump::None => {}
            Bump::Patch => next.patch += 1,
            Bump::Minor => {
                next.minor += 1;
                next.patch = 0;
            }
            Bump::Major => {
                next.major += 1;
                next.minor = 0;
                next.patch = 0;
            }
        }
    }
    let Some(tag) = prerelease else {
        return Ok(next.to_string());
    };
    let same_base =
        (next.major, next.minor, next.patch) == (version.major, version.minor, version.patch);
    let count = match counted {
        Some(counted) if counted.same_tag && same_base => counted.count + 1,
        Some(_) | None => 1,
    };
    next.pre = semver::Prerelease::new(&format!("{}.{count}", tag.as_str()))
        .with_context(|| format!("prerelease {:?}", tag.as_str()))?;
    ensure!(
        next > version,
        "{next} sorts below the current {current}; set `prerelease` to a tag that sorts above \
         the current one, or remove it to release the stable version"
    );
    Ok(next.to_string())
}

/// Whether a prerelease `version` already holds the base `bump` would reach.
fn reaches(version: &Version, bump: Bump) -> bool {
    match bump {
        Bump::None | Bump::Patch => true,
        Bump::Minor => version.patch == 0,
        Bump::Major => version.minor == 0 && version.patch == 0,
    }
}

/// A current prerelease read in the shape verctl writes, `<tag>.<count>`.
struct Counted {
    /// Whether the tag is the one the package sets.
    same_tag: bool,
    /// The count after the tag.
    count: u64,
}

impl Counted {
    fn read(version: &Version, tag: &Prerelease) -> Result<Self> {
        let pre = version.pre.as_str();
        let shaped = pre
            .split_once('.')
            .filter(|(name, _)| {
                !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_lowercase())
            })
            .and_then(|(name, count)| Some((name, count.parse::<u64>().ok()?)));
        let Some((name, count)) = shaped else {
            let base = format!("{}.{}.{}", version.major, version.minor, version.patch);
            bail!(
                "{version} has a prerelease verctl cannot count from; verctl writes \
                 `<tag>.<count>`, such as {base}-{}.2: change the version in the manifest to that \
                 shape",
                tag.as_str()
            );
        };
        Ok(Self {
            same_tag: name == tag.as_str(),
            count,
        })
    }
}
