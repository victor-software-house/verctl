## ADDED Requirements

### Requirement: `check` runs the checks it names

`verctl check [CHECK]...` SHALL run each named check, from `fragments`, `served`, `versions`, `registry` and `trust`, and SHALL report each one's result.
Names SHALL add up and never overlap, and each check SHALL run once however often its name appears.

With no name, `check` SHALL run `fragments` and `served`.
These need only the checkout, with no network and no token.
`--all` (`-a`) SHALL run every check.
An unknown name SHALL fail the parse and list the known names.

#### Scenario: Bare check

- **WHEN** `verctl check` runs with no name and no network
- **THEN** it runs `fragments` and `served`, sends no request, and exits zero when both pass

#### Scenario: Two names

- **WHEN** `verctl check versions registry` runs in CI
- **THEN** it runs `versions` and `registry` only, and reports each one's result

#### Scenario: Every check

- **WHEN** `verctl check --all` runs with no owner's token in `VERCTL_REGISTRY_TOKEN`
- **THEN** it runs all five checks, and `trust` fails naming `VERCTL_REGISTRY_TOKEN`

#### Scenario: An unknown name

- **WHEN** `verctl check release` runs
- **THEN** it exits 2 and lists `fragments`, `served`, `versions`, `registry` and `trust`

### Requirement: `check` takes no `--versions` flag

`verctl check` SHALL NOT accept `--versions`, and SHALL NOT read `VERCTL_VERSIONS`.
The comparison of manifests with the default branch SHALL run only as the `versions` check.

#### Scenario: The removed flag

- **WHEN** `verctl check --versions` runs
- **THEN** it exits 2 with clap's unknown-argument error, and runs no check

### Requirement: The `registry` check proves the registry side without a token

`verctl check registry` SHALL prove, for each crate the repository publishes to crates.io, that the crate exists, and that its owners include every login its `owners` lists.
Without any `owners` for a crate it SHALL skip the owner proof for that crate and say so.

It SHALL report, as evidence and not as proof, whether the crate's latest version came through trusted publishing from the repository.
It SHALL exit non-zero naming each crate and each failed proof.

#### Scenario: Every crate is ready

- **WHEN** `.ctl/ver.yaml` sets `owners: [any-victor]`, each crate exists and lists `any-victor` as an owner, and `verctl check registry` runs with no token
- **THEN** it exits zero, and reports for each crate whether its latest version came through trusted publishing from the repository

#### Scenario: An owner is missing

- **WHEN** `.ctl/ver.yaml` sets `owners: [any-victor]` and crate `demo` lists only `alice` as an owner
- **THEN** `verctl check registry` exits non-zero, names `demo`, the missing login `any-victor`, and the owners it found

### Requirement: A crate new in the pull request waits for its bootstrap

The `registry` check SHALL apply the existence proof to the crates the default branch holds.
A crate that only the pull request adds SHALL pass it, with a notice that it needs `verctl release bootstrap` once it lands.

#### Scenario: A new crate on a pull request

- **WHEN** a pull request adds workspace member `demo-extra` at `0.0.0`, the default branch holds no `demo-extra`, crates.io has none, and `verctl check registry` runs in CI
- **THEN** it exits zero, with a notice that `demo-extra` needs `verctl release bootstrap` once it lands

#### Scenario: A crate the default branch holds and the registry lacks

- **WHEN** the default branch holds `demo-extra`, crates.io has none, and `verctl check registry` runs on any pull request
- **THEN** it exits non-zero, names `demo-extra` as missing from crates.io, and names `verctl release bootstrap demo-extra` as the fix

### Requirement: The `registry` check repeats every request on a rerun

The `registry` check SHALL need the network and no token.
A rerun SHALL repeat every request whatever the state of the pull request.

#### Scenario: A rerun repeats the proof

- **WHEN** `verctl check registry` failed on `demo-extra`, which the default branch holds, the operator ran `verctl release bootstrap demo-extra` at the default branch's tip, and the CI job reruns with "Re-run failed jobs"
- **THEN** the rerun queries crates.io again and exits zero

### Requirement: The `trust` check proves the trusted publisher with an owner's token

`verctl check trust` SHALL read `VERCTL_REGISTRY_TOKEN`, an owner's token with the `trusted-publishing` scope, and SHALL prove for each crate that a trusted publisher entry names the repository's release workflow.
Without the token it SHALL fail and name the variable.

#### Scenario: A missing entry

- **WHEN** `VERCTL_REGISTRY_TOKEN` holds an owner's token, and crate `demo` has no trusted publisher entry naming `release.yml`
- **THEN** `verctl check trust` exits non-zero and names `demo` as lacking a trusted publisher for `release.yml`

#### Scenario: No token

- **WHEN** `VERCTL_REGISTRY_TOKEN` is unset and `verctl check trust` runs
- **THEN** it exits non-zero, names `VERCTL_REGISTRY_TOKEN`, and sends no request to `/api/v1/trusted_publishing/github_configs`

### Requirement: `release --dry-run` proves a release and uploads nothing

`verctl release --dry-run` (`-n`) SHALL run the `fragments`, `served`, `versions` and `registry` checks, and `trust` when `VERCTL_REGISTRY_TOKEN` holds a token.
It SHALL also run `cargo publish --workspace --dry-run`, replay `verctl version` and compare the result with the commit, and build every declared asset.
It SHALL upload nothing, and change no registry and no Release.

#### Scenario: The Version PR in a merge group

- **WHEN** the Version PR's merge-group commit holds `demo` `0.4.0` and `verctl release --dry-run` runs on it
- **THEN** it runs the checks, `cargo publish --workspace --dry-run`, the replay of `verctl version`, and each asset build
- **AND** it exits zero when all of them pass, and creates no draft Release

#### Scenario: A pull request changed what the Version PR releases

- **WHEN** a pull request merged after the Version PR was prepared adds a fragment, and `verctl release --dry-run` runs on the Version PR's head
- **THEN** the replay of `verctl version` differs from the commit, and the run exits non-zero naming the differing file

#### Scenario: A package that does not build

- **WHEN** `cargo publish --workspace --dry-run` fails for crate `app`
- **THEN** `verctl release --dry-run` exits non-zero, names `app`, and uploads nothing

### Requirement: `release` runs the same proof before its first upload

`verctl release` SHALL run the proof of `release --dry-run` before its first upload.
When it fails, `verctl release` SHALL exit non-zero and SHALL publish, tag, and release nothing.

#### Scenario: Drift between the merge and the release

- **WHEN** crate `demo` lost owner `any-victor` after the Version PR merged, and `verctl release` runs from the push that landed it
- **THEN** the `registry` check fails, the run exits non-zero naming `demo`, and no crate, tag, or Release is published
