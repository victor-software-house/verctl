# Tasks

The pull request's CI verifies every compile, format, lint, and test task.
This change is one part of [release-state-machine](../release-state-machine/tasks.md), which records the operator's decisions in its section 0 and releases 0.8.0 in its section 5.
Tasks keep the numbers they had there.
State notes are from the code on `main` at [cb1be28][main], read on 2026-11-09 (UTC).

## 1. Blocks and built-ins

- [ ] 1.1 The Manifest, Lockfile, Artifact, and Registry blocks exist, with cargo and bun providers for version, dependents, lockfile refresh through the owner, lockfile check by parsing, publish order, and registry query. Proof: the building-blocks and built-in-ecosystems cargo and bun scenarios pass as tests
  - 2026-11-09, partly shipped: version read and write through drivers, the registry query before and after each publish ([#162][pr162]), and cargo workspace members in dependency order (`src/members.rs`). Remaining: the block model, lockfile refresh through the owner (verctl prints the follow-up command), lockfile check by parsing, and publish order across packages (VER-033).

## 2. The release state machine

- [ ] 2.3 bun publishes through `bun pm pack`, the packed-manifest inspection, and verctl's registry client with `auth: token`; proof: the built-in-ecosystems bun scenarios pass against a local npm-compatible registry
  - 2026-11-09, not started.

## 6. Patches after 0.8.0

- [ ] 6.1 `auth: oidc` for crates.io and npm; proof: the built-in-ecosystems OIDC scenarios pass, and one real crates.io release runs with no `CARGO_REGISTRY_TOKEN` secret
  - 2026-11-09, partly shipped: `release.yml` publishes to crates.io through trusted publishing with `crates-io-trusted-publishing` ([#218][pr218]). Remaining: npm OIDC, and `auth: oidc` as a config key.
- [ ] 6.2 `provenance: true`, staged publish, and `release bootstrap` in the bun registry client; proof: one real bun-release version publishes with provenance through `verctl release`, and bun-release is archived
  - 2026-11-09, not started.
- [ ] 6.4 The built-in `mise-pin` step, and `MISE_LOCKED_VERIFY_PROVENANCE=1` on `mise install --locked` in the served CI workflow. Proof: the built-in-ecosystems pin scenarios pass, a consumer's lock regenerated after 0.8.x records `provenance = "github-attestations"`, and that consumer's CI install passes with the variable set
  - 2026-11-09, not started.
- [ ] 6.5 A first publish works beside trusted publishing. The publish plan reports, for each crates.io package, whether the crate exists. The reusable `release.yml` accepts `registry-token` together with `crates-io-trusted-publishing`. It publishes a crate that does not exist yet with the token, and every other crate through trusted publishing. Proof: a plan test with a mocked sparse index reports a missing crate, and one real first publish of a new crate passes under a caller that sets both
  - 2026-11-09, not started. crates.io refused a new crate under trusted publishing alone, because trusted publishing is configured on a crate that already exists. Today `release.yml` refuses the two together ([`release.yml:314`][release-yml]). Patch-safe: the plan gains a field, and a combination that fails today starts to work.

[main]: https://github.com/victor-software-house/verctl/commit/cb1be28
[pr162]: https://github.com/victor-software-house/verctl/pull/162
[pr218]: https://github.com/victor-software-house/verctl/pull/218
[release-yml]: https://github.com/victor-software-house/verctl/blob/cb1be28/.github/workflows/release.yml#L314
