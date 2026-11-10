//! The registry token `release` hands to each publisher.
//!
//! The release workflow sets the token in one of two variables and runs the
//! bare verb. verctl reads it once, here, and gives it to each publisher under
//! the variable `--registry-token-env` names, and to the publisher's registry
//! `query`, which needs the same credential for a private registry. No other
//! child process sees it: [`crate::process`] removes both source variables
//! from every command it starts, and only those two get the token back under
//! its own name.

use std::{env, fmt};

use anyhow::{Result, bail};

/// The variable the release workflow sets from its `registry-token` secret.
pub const SECRET: &str = "VERCTL_REGISTRY_TOKEN";

/// The variable the release workflow sets from a crates.io trusted-publishing
/// token.
pub const TRUSTED: &str = "VERCTL_TRUSTED_PUBLISHING_TOKEN";

/// The variables a token arrives in, which no child process inherits.
pub const SOURCES: [&str; 2] = [SECRET, TRUSTED];

/// A registry token and the variable its publisher reads it from.
///
/// Its `Debug` output names the variable and hides the token, so an error or a
/// log line that prints it shows no secret.
#[derive(Clone, PartialEq, Eq)]
pub struct RegistryToken {
    variable: String,
    value: String,
}

impl fmt::Debug for RegistryToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegistryToken")
            .field("variable", &self.variable)
            .finish_non_exhaustive()
    }
}

impl RegistryToken {
    /// The token in [`SECRET`] or [`TRUSTED`], for the publisher to read
    /// from `variable`. `None` when neither is set; an empty variable is
    /// unset.
    ///
    /// # Errors
    ///
    /// Returns an error when both are set, because one release takes one
    /// credential and picking either would hide a mistake in the caller.
    pub fn from_env(variable: &str) -> Result<Option<Self>> {
        let read = |key| {
            env::var(key)
                .ok()
                .filter(|value: &String| !value.is_empty())
        };
        Self::resolve(read(SECRET), read(TRUSTED), variable)
    }

    /// [`Self::from_env`] over values already read.
    ///
    /// # Errors
    ///
    /// Returns an error when both `secret` and `trusted` hold a token.
    pub(crate) fn resolve(
        secret: Option<String>,
        trusted: Option<String>,
        variable: &str,
    ) -> Result<Option<Self>> {
        let value = match (secret, trusted) {
            (Some(_), Some(_)) => bail!(
                "{SECRET} and {TRUSTED} are both set: pass either the registry-token secret or crates-io-trusted-publishing"
            ),
            (Some(value), None) | (None, Some(value)) => value,
            (None, None) => return Ok(None),
        };
        Ok(Some(Self {
            variable: variable.to_owned(),
            value,
        }))
    }

    /// The one variable the publisher runs with: its name and the token.
    #[must_use]
    pub fn env(&self) -> (&str, &str) {
        (&self.variable, &self.value)
    }
}

#[cfg(test)]
mod tests {
    use super::{RegistryToken, SECRET, TRUSTED};

    #[test]
    fn both_tokens_fail_and_name_both_variables() {
        let error = RegistryToken::resolve(
            Some("from-secret".into()),
            Some("from-trusted".into()),
            "CARGO_REGISTRY_TOKEN",
        )
        .expect_err("both set");
        let message = format!("{error:#}");
        assert!(message.contains(SECRET), "{message}");
        assert!(message.contains(TRUSTED), "{message}");
        assert!(!message.contains("from-"), "{message}");
    }

    #[test]
    fn either_token_goes_under_the_named_variable() {
        for (secret, trusted) in [(Some("t0ken".into()), None), (None, Some("t0ken".into()))] {
            let token = RegistryToken::resolve(secret, trusted, "CARGO_REGISTRIES_DEMO_TOKEN")
                .expect("one token")
                .expect("set");
            assert_eq!(token.env(), ("CARGO_REGISTRIES_DEMO_TOKEN", "t0ken"));
        }
    }

    #[test]
    fn no_token_is_none() {
        assert_eq!(
            RegistryToken::resolve(None, None, "CARGO_REGISTRY_TOKEN").expect("none"),
            None
        );
    }

    #[test]
    fn debug_hides_the_token() {
        let token = RegistryToken::resolve(Some("t0ken".into()), None, "CARGO_REGISTRY_TOKEN")
            .expect("one token")
            .expect("set");
        let shown = format!("{token:?}");
        assert!(shown.contains("CARGO_REGISTRY_TOKEN"), "{shown}");
        assert!(!shown.contains("t0ken"), "{shown}");
    }
}
