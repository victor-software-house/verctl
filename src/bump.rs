use crate::fragment::Bump;
use anyhow::{Context, Result, bail};
use semver::Version;

/// The version `bump` moves `current` to.
///
/// Two bumps are refused. A `major` on any 0.x package, because 1.0 is a
/// decision rather than a fragment. A `minor` or `major` on a package at
/// exactly `0.0.0`, because a new package releases `0.0.1` first; opening at
/// `0.1.0` is the operator's call, recorded as `first_minor` on the package.
pub fn apply(current: &str, bump: Bump, first_minor: bool) -> Result<String> {
    if bump == Bump::None {
        return Ok(current.to_owned());
    }
    let mut version = Version::parse(current).with_context(|| format!("semver {current:?}"))?;
    if version == Version::new(0, 0, 0) && bump > Bump::Patch && !first_minor {
        bail!(
            "0.0.0 refuses a {} bump: a new package releases 0.0.1 first, from a patch \
             fragment; set `first_minor: true` on the package to open at 0.1.0",
            bump.as_str()
        );
    }
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
    version.pre = semver::Prerelease::EMPTY;
    version.build = semver::BuildMetadata::EMPTY;
    Ok(version.to_string())
}
