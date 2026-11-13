# Design

## 1. Retry once, only when every rejected crate is trusted

**Decision.**
`verctl semver` runs `cargo semver-checks` as today, and on a failure takes these steps:

1. It reads the run's stderr for cargo's cooldown rejections: a `failed to select a version for the requirement` line naming a crate, followed by `is too new`.
2. For each crate so named, it reads `GET https://crates.io/api/v1/crates/<name>/owners`.
3. If each such crate has a user owner whose login `CARGO_TRUSTED_OWNERS` lists, it runs the same command once more with `CARGO_RESOLVER_INCOMPATIBLE_PUBLISH_AGE=allow`.
   The second run decides the crate's row.

**Alternative.**
Always run `cargo semver-checks` with `CARGO_RESOLVER_INCOMPATIBLE_PUBLISH_AGE=allow`.
It lost because it drops the cooldown for every crate in every check, including a third-party release published minutes before.
The retry keeps a first run under the cooldown, and runs without it only when our own releases are what blocks the check.

**Cost.**
Cargo has no per-crate exemption, so the second run resolves every dependency without the cooldown, third-party crates included.
That run only builds rustdoc output for the API comparison; `Cargo.lock`, builds, tests and releases keep the cooldown.

## 2. The trusted logins come from `CARGO_TRUSTED_OWNERS`

**Decision.**
verctl reads the space-separated logins in `CARGO_TRUSTED_OWNERS`, the variable the templates repository renders and the actions repository's `update:own` task reads.
A team owner (`kind` `team`) never matches; only a user login does.

**Alternative.**
A verctl config key for trusted owners.
It lost because the repositories already declare the list once, in `mise.toml`'s `[env]`, and a second list would drift.

**Override and disable.**
With `CARGO_TRUSTED_OWNERS` unset or empty, verctl never retries, and the check fails as before.
A repository turns the cooldown itself off through the templates repository's `cargo_min_publish_age` answer.

## 3. Reading cargo's message

**Decision.**
verctl matches cargo 1.100's wording, which `run_inherit` already keeps in its stderr tail.
A message verctl does not recognise leaves the check failed as before, so a change in cargo's wording costs a retry, never a false pass.

**Alternative.**
Resolve the placeholder project itself first, to list the too-new crates.
It lost because cargo-semver-checks builds that project internally, and verctl would have to repeat its manifest generation.
