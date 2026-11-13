## ADDED Requirements

### Requirement: `check --registry` proves the registry side on every pull request

`verctl check --registry` SHALL prove, for each crate the repository publishes to crates.io, that the crate exists, and that its owners include every login the `owners` key of `.ctl/ver.yaml` lists.
Without the `owners` key it SHALL skip the owner proof and say so.

It SHALL need the network, and a rerun SHALL repeat every request whatever the state of the pull request.
It SHALL exit non-zero and name each crate and each failed proof.

#### Scenario: A new crate on a pull request

- **WHEN** a pull request adds workspace member `demo-extra` at `0.0.0`, crates.io has no `demo-extra`, and `verctl check --registry` runs in CI
- **THEN** it exits non-zero and names `demo-extra` as missing from crates.io

#### Scenario: A rerun repeats the proof

- **WHEN** `verctl check --registry` failed on `demo-extra`, the operator ran `verctl release --first` for it, and the CI job reruns with "Re-run failed jobs"
- **THEN** the rerun queries crates.io again and exits zero

#### Scenario: Every crate is ready, without a token

- **WHEN** `.ctl/ver.yaml` sets `owners: [any-victor]`, each crate exists and lists `any-victor` as an owner, `VERCTL_REGISTRY_TOKEN` is unset, and `verctl check --registry` runs
- **THEN** it exits zero, and reports for each crate whether its latest version came through trusted publishing from the repository, with the trusted publisher marked as not checked

#### Scenario: An owner is missing

- **WHEN** `.ctl/ver.yaml` sets `owners: [any-victor]` and crate `demo` lists only `alice` as an owner
- **THEN** `verctl check --registry` exits non-zero, names `demo`, the missing login `any-victor`, and the owners it found

### Requirement: `check --registry` proves the trusted publisher only with an owner's token

With `VERCTL_REGISTRY_TOKEN` set to an owner's token, `verctl check --registry` SHALL prove that a trusted publisher config names the repository's release workflow.
Without it, the check SHALL report as evidence whether the crate's latest version was published by trusted publishing from the repository, and SHALL state that the trusted publisher was not checked.

#### Scenario: With an owner's token

- **WHEN** `VERCTL_REGISTRY_TOKEN` holds an owner's token with the trusted-publishing scope, and crate `demo` has no trusted publisher config naming `release.yml`
- **THEN** `verctl check --registry` exits non-zero and names `demo` as lacking a trusted publisher for `release.yml`

### Requirement: `check --release` proves a release before it lands

`verctl check --release` SHALL run every proof of `check --registry`.
It SHALL also run `cargo publish --workspace --dry-run`, replay `verctl version` and compare the result with the commit, and build every declared asset and upload it to a draft Release.
It SHALL change no registry.

#### Scenario: The Version PR in a merge group

- **WHEN** the Version PR's merge-group commit holds `demo` `0.4.0` and `verctl check --release` runs on it
- **THEN** it runs `cargo publish --workspace --dry-run`, replays `verctl version`, and uploads each asset to a draft Release for `v0.4.0`
- **AND** it exits zero when all of them pass

#### Scenario: A pull request changed what the Version PR releases

- **WHEN** a pull request merged after the Version PR was prepared adds a fragment, and `verctl check --release` runs on the Version PR's head
- **THEN** the replay of `verctl version` differs from the commit, and the check exits non-zero naming the differing file

#### Scenario: A package that does not build

- **WHEN** `cargo publish --workspace --dry-run` fails for crate `app`
- **THEN** `verctl check --release` exits non-zero, names `app`, and publishes nothing

### Requirement: `release` runs `check --release` before its first upload

`verctl release` SHALL run `check --release` before its first upload.
When it fails, `verctl release` SHALL exit non-zero and SHALL publish, tag, and release nothing.

#### Scenario: Drift between the merge and the release

- **WHEN** a crate's trusted publisher config was removed after the Version PR merged, and `verctl release` runs from the push that landed it
- **THEN** `check --release` fails, the run exits non-zero naming the crate, and no crate, tag, or Release is published

### Requirement: The default branch freezes while a release is incomplete

While the default branch holds a version whose release is incomplete, `verctl check --registry` SHALL fail on every other pull request or merge group, and SHALL name the pending release.
It SHALL first wait for a running release workflow run at the default branch's tip, within its timeout, and SHALL decide after that run ends.

#### Scenario: A pull request while a release is partial

- **WHEN** `main` holds `demo` `0.4.0`, crates.io does not list it, no release run is active, and `verctl check --registry` runs on another pull request
- **THEN** it exits non-zero and names `demo@0.4.0` as the pending release

#### Scenario: A pull request during a normal release

- **WHEN** `main` holds `demo` `0.4.0`, a `release.yml` run at the tip of `main` is in progress, and `verctl check --registry` runs on another pull request
- **THEN** it waits for that run
- **AND** when the run ends with `demo` `0.4.0` listed on crates.io and tag `v0.4.0` published, the check exits zero

#### Scenario: The wait runs out

- **WHEN** the `release.yml` run at the tip of `main` is still running when the timeout ends
- **THEN** `verctl check --registry` exits non-zero and names the running release and the timeout

### Requirement: The `verctl: unfreeze` label passes the freeze

A pull request that carries the label `verctl: unfreeze` SHALL pass the freeze.
The label SHALL pass only the freeze; every other proof of `check --registry` SHALL still apply.

#### Scenario: A release that can never complete

- **WHEN** crate `demo` `0.4.0` can never publish, because another owner took the name, and a pull request labelled `verctl: unfreeze` runs `verctl check --registry`
- **THEN** the freeze does not fail the check, and the check names the pending release as passed by the label

#### Scenario: The label does not pass a registry proof

- **WHEN** a pull request labelled `verctl: unfreeze` adds crate `demo-extra`, which crates.io lacks
- **THEN** `verctl check --registry` exits non-zero naming `demo-extra`

### Requirement: A run releases HEAD's versions only, from HEAD

A release run SHALL release the versions HEAD's manifests hold, from HEAD.
`verctl status` SHALL list HEAD's incomplete releases only.
The reusable `release.yml` SHALL check out no commit other than the one that triggered it.

#### Scenario: A version HEAD no longer holds

- **WHEN** commit `A` set `core` to `0.3.1` and was never released, a later commit `C` set `core` to `0.4.0`, HEAD is `C`, and `verctl status` runs
- **THEN** it lists `core@0.4.0` and does not list `core@0.3.1`

#### Scenario: The workflow checks out one commit

- **WHEN** `release.yml` runs on a push to `main`
- **THEN** every `actions/checkout` step in it checks out `github.sha` and no job checks out another ref

### Requirement: A partial release is its own state

`verctl status` SHALL report a release with some but not all registry uploads as a partial release, with its count and its cause.
A rerun, by dispatch or by "Re-run failed jobs", SHALL skip every crate the registry index lists, and SHALL resume with the rest.

#### Scenario: One of nine crates is live

- **WHEN** `shrub-parser` `0.7.0` is on crates.io, the other 8 crates are not, and crates.io refused `shrub-process` with status 429 and a `Retry-After` header
- **THEN** `verctl status` reports `partially published: 1 of 9 crates live`, names the 429 as the cause, and names the resume time from the header

#### Scenario: A rerun resumes

- **WHEN** the cause is gone and `verctl release` reruns
- **THEN** it runs no `cargo publish` for `shrub-parser`, publishes the other 8 crates in dependency order, and then publishes the draft Release

### Requirement: `status` names the enforcement tier

`verctl status` SHALL name the enforcement tier, read from the default branch's rules, as one of these:

1. `merge queue`: a merge queue applies to the default branch
2. `rulesets without a queue`: rulesets apply and no merge queue does
3. `neither`: no ruleset or merge queue applies, and the freeze is reported, not enforced

#### Scenario: A merge queue

- **WHEN** the default branch's rules include a merge queue and `verctl status` runs
- **THEN** it reports the tier `merge queue`

#### Scenario: Rulesets without a queue

- **WHEN** the default branch's rules include a ruleset requiring `check --release` on pull requests and no merge queue
- **THEN** `verctl status` reports `rulesets without a queue`

#### Scenario: Neither

- **WHEN** the default branch has no ruleset and no merge queue
- **THEN** `verctl status` reports `neither` and states that the freeze is not enforced
