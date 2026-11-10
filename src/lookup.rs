//! Lookups that ask something outside the commit for its state: GitHub, a
//! registry, or history a shallow clone does not hold.

use std::error::Error;
use std::fmt;

/// A lookup that could not run: the walk reached a shallow clone's boundary,
/// the repository names no GitHub repository, or GitHub or a registry did not
/// answer. It says nothing about the release itself, so `status` reports the
/// check as not run. Any other error, such as a release tag on the wrong
/// commit or a config that does not resolve, stays an error.
///
/// It displays as the error it wraps and continues that error's chain, so
/// marking an error changes no message.
#[derive(Debug)]
pub struct Unavailable(anyhow::Error);

impl fmt::Display for Unavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for Unavailable {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.0.source()
    }
}

/// Mark `error` as a lookup that could not run.
#[must_use]
pub fn unavailable(error: anyhow::Error) -> anyhow::Error {
    anyhow::Error::new(Unavailable(error))
}

/// Whether `error`, or a cause under its context, is a lookup that could not
/// run.
#[must_use]
pub fn is_unavailable(error: &anyhow::Error) -> bool {
    error.chain().any(<dyn Error>::is::<Unavailable>)
}

#[cfg(test)]
mod tests {
    use anyhow::{Context, anyhow};

    use super::{is_unavailable, unavailable};

    /// Marking keeps the message and the chain, and context added later does
    /// not hide the mark.
    #[test]
    fn a_marked_error_keeps_its_chain_under_context() {
        let marked = unavailable(anyhow!("no answer").context("get release v1.0.0"));
        let wrapped = Err::<(), _>(marked)
            .context("read demo")
            .expect_err("error");
        assert!(is_unavailable(&wrapped));
        assert_eq!(
            format!("{wrapped:#}"),
            "read demo: get release v1.0.0: no answer"
        );
    }

    #[test]
    fn an_unmarked_error_is_not_a_lookup() {
        let error = anyhow!("tag v1.0.0 names another commit");
        assert!(!is_unavailable(&error));
    }
}
