use crate::fragment::Bump;
use anyhow::{Context, Result, bail};
use semver::Version;

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
