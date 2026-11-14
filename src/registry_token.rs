//! The registry token `release` hands to each publisher.
//!
//! The release workflow sets the token in one of two variables and runs the
//! bare verb.
//! verctl reads it once, here.
//! It gives the token to each publisher under the variable
//! `--registry-token-env` names.
//! The publisher's registry `query` gets it too, because a private registry
//! needs the same credential for that query.
//!
//! No other child process sees it.
//! [`crate::process`] removes both source variables, and the owner's token in
//! [`OWNER`], from every command it starts, and only the publisher and its
//! query get the registry token back under the named variable.

use std::{env, fmt};

use anyhow::{Result, bail};

/// The variable the release workflow sets from its `registry-token` secret.
pub const SECRET: &str = "VERCTL_REGISTRY_TOKEN";

/// The variable the release workflow sets from a crates.io trusted-publishing
/// token.
pub const TRUSTED: &str = "VERCTL_TRUSTED_PUBLISHING_TOKEN";

/// The variables a token arrives in, which no child process inherits.
pub const SOURCES: [&str; 2] = [SECRET, TRUSTED];

/// The variable an owner's crates.io token with the `trusted-publishing`
/// scope arrives in, for `check trust` and the release proof to read the
/// trusted publisher entries. It is not the token `release` publishes with:
/// that scope can also create and delete entries, so no publisher gets it,
/// and no child process inherits it.
pub const OWNER: &str = "VERCTL_OWNER_TOKEN";

/// The owner's token in [`OWNER`], or `None` when it is unset or empty.
#[must_use]
pub fn owner_token() -> Option<String> {
    env::var(OWNER).ok().filter(|token| !token.is_empty())
}

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

    /// The token in [`SECRET`] alone, for the publisher to read from
    /// `variable`. `None` when it is unset or empty.
    ///
    /// `release bootstrap` takes the operator's token and never a
    /// trusted-publishing token, because a trusted publisher cannot create a
    /// crate.
    #[must_use]
    pub fn operator(variable: &str) -> Option<Self> {
        Self::resolve(
            env::var(SECRET).ok().filter(|value| !value.is_empty()),
            None,
            variable,
        )
        .ok()
        .flatten()
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
