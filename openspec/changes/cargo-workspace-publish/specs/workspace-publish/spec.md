## ADDED Requirements

### Requirement: A virtual workspace publishes member by member

A package on the stock cargo publisher, with no `publish` or `query` override
on the package or on `publishers.cargo`, whose `path` is a manifest with a
`[workspace]` table and no `[package]` table, SHALL be published by running
`cargo publish --locked --manifest-path <path> --package <member>` once for
each workspace member whose `publish` is not `false`, each under its own
timeout. A member with `publish = false` SHALL never be published.

#### Scenario: Two crates and a tool crate

- **WHEN** `.ctl/ver.yaml` declares `demo` at `Cargo.toml`, a virtual manifest whose members are `core`, `app`, and `xtask`, and `xtask` sets `publish = false`
- **THEN** `verctl publish --dry-run` lists `core` and `app` and not `xtask`

#### Scenario: An overridden upload keeps one command

- **WHEN** the same package declares `publish: [cargo, publish, --locked, --workspace]`
- **THEN** `verctl publish` runs that one command, as before this change

### Requirement: Members publish in dependency order

A member SHALL be published after every published member it depends on
through a normal or build dependency, or through a development dependency
that declares a version. A path-only development dependency SHALL NOT order
publishes. Members with no such dependency between them SHALL keep the order
of `workspace_members` in `cargo metadata`. A cycle among the dependencies
that order publishes SHALL fail before any upload and name the cycle.

#### Scenario: A dependent listed first

- **WHEN** the members are listed as `app`, `core`, and `app` has a normal dependency on `core`
- **THEN** `verctl publish --dry-run` lists `core` before `app`

#### Scenario: A test-only path dependency

- **WHEN** `core` has a path-only development dependency on `app`, and `app` has a normal dependency on `core`
- **THEN** `verctl publish --dry-run` lists `core` before `app`

#### Scenario: Two crates that test with each other

- **WHEN** `a` and `b` each have a development dependency on the other that declares `version = "0.1"`
- **THEN** `verctl publish` exits non-zero before any upload and names `a -> b -> a`

### Requirement: The index decides which members are published

For a member bound for crates.io, `verctl publish` SHALL read the member's
crates.io index file before its upload, and SHALL skip the upload and report
the member `already` when the index lists the member's version. The package
SHALL count as published only when every published member is listed.

#### Scenario: A rerun after a partial release

- **WHEN** crates.io lists `core` `0.0.2` but not `app` `0.0.2`, and `verctl publish` runs at the version commit
- **THEN** it reports `core` as `already`, runs `cargo publish` only for `app`, and creates the tag after `app` is listed

#### Scenario: Nothing left

- **WHEN** crates.io lists every published member at `0.0.2`, and the tag `v0.0.2` and its Release exist
- **THEN** `verctl publish --plan` reports `nothing to release`

### Requirement: A rate limit stops the release without a tag

When a member's upload fails with status 429, `verctl publish` SHALL exit
non-zero, name the member, and repeat the time crates.io gives after "try
again after". It SHALL create no tag and no Release in that run.

#### Scenario: The sixth new crate

- **WHEN** `cargo publish` for `app` fails with `the remote server responded with an error (status 429 Too Many Requests): You have published too many new crates in a short period of time. Please try again after Sat, 19 Oct 2026 01:23:45 GMT or email help@crates.io to have your limit increased.`
- **THEN** `verctl publish` exits non-zero with a message that names `app@0.0.2` and `Sat, 19 Oct 2026 01:23:45 GMT`, and no tag `v0.0.2` exists

### Requirement: docs waits for every library member

`verctl docs` SHALL wait for each published member bound for crates.io that
has a library target, at the member's version, when the package's tag names
HEAD.

#### Scenario: A binary member

- **WHEN** members `core` (a library) and `app` (a binary only) are released at the tag on HEAD
- **THEN** `verctl docs` waits for `core` and not for `app`
