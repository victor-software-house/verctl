//! Lookups that ask something outside the commit for its state: GitHub, a
//! registry, or history a shallow clone does not hold.

use std::error::Error;
use std::fmt;

use reqwest::header::HeaderMap;

/// A lookup that could not run. Three cases qualify:
///
/// - The walk reached a shallow clone's boundary.
/// - The repository names no GitHub repository.
/// - GitHub or a registry gave no answer: a failed connection, a timeout, a
///   5xx, or a rate limit (see [`no_answer`]).
///
/// Such an error shows nothing about the release itself, so `status` reports
/// the check as not run. Any other error stays an error. Examples are a
/// release tag on the wrong commit, a 401 from a bad token, and a config that
/// does not resolve. The module that makes the request marks it. Its caller
/// never does.
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

/// Whether an HTTP answer with `status` says only that the server cannot
/// answer now: a 5xx, a 429, or a 403 whose `x-ratelimit-remaining` header in
/// `headers` is `0`. GitHub answers its primary rate limit with that 403 or a
/// 429, as its [rate limit docs] say; any other 403 is a refusal.
///
/// [rate limit docs]: https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api#exceeding-the-primary-rate-limit
#[must_use]
pub fn no_answer(status: reqwest::StatusCode, headers: Option<&HeaderMap>) -> bool {
    status.is_server_error()
        || status == reqwest::StatusCode::TOO_MANY_REQUESTS
        || (status == reqwest::StatusCode::FORBIDDEN
            && headers
                .and_then(|headers| headers.get("x-ratelimit-remaining"))
                .is_some_and(|remaining| remaining.as_bytes() == b"0"))
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
