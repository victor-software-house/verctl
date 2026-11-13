# Tasks

## 1. Retry past the cooldown

- [x] 1.1 Parse cargo's cooldown rejections from the `cargo semver-checks` stderr tail into crate names. Proof: unit tests in `src/semver_check.rs` on recorded cargo 1.100 output, one name and several, and on output with no rejection.
- [x] 1.2 Read `crates/<name>/owners` from crates.io and match user logins against `CARGO_TRUSTED_OWNERS`. Proof: unit tests on recorded owners responses, a team owner, and an unset variable.
- [x] 1.3 Run the check once more with `CARGO_RESOLVER_INCOMPATIBLE_PUBLISH_AGE=allow` when every rejected crate is trusted, and name the rejected crates and the reason in the error otherwise. Proof: tests of the decision with a stub command runner, and `mise run lint` and `mise run test` in CI.

## 2. Docs

- [x] 2.1 `src/instructions.md`, under `semver`, says when the check runs again and that it reads `CARGO_TRUSTED_OWNERS`. Proof: `rg CARGO_TRUSTED_OWNERS src/instructions.md`.
