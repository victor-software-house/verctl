## ADDED Requirements

### Requirement: verctl reads crate owners and trusted publishers

For each crate that publishes to crates.io, verctl SHALL read the crate's owners from `GET /api/v1/crates/{name}/owners`, which needs no token.

It SHALL read the trusted publisher entries from `GET /api/v1/trusted_publishing/github_configs?crate={name}` only with an owner's token, because crates.io serves them to owners alone.
It SHALL name each crate that is missing, lacks a listed owner, or, with that token, lacks a trusted publisher for the release workflow.

#### Scenario: A crate the registry lacks

- **WHEN** workspace member `shrub-process` is at `0.7.0` and `GET /api/v1/crates/shrub-process/owners` answers 404
- **THEN** verctl reports `shrub-process` as missing from crates.io

#### Scenario: Owners against the declared list

- **WHEN** `.ctl/ver.yaml` sets `owners: [any-victor]` and `GET /api/v1/crates/demo/owners` lists only `alice`
- **THEN** verctl reports `demo` as lacking owner `any-victor`, and names the owners it found

#### Scenario: Trusted publishers without a token

- **WHEN** no owner's token is set
- **THEN** verctl sends no request to `/api/v1/trusted_publishing/github_configs`, and reports the trusted publisher as not checked

### Requirement: `owners` names the expected owners once, with a per-package override

The top-level `owners` key of `.ctl/ver.yaml` SHALL list the crates.io logins every package must have as owners.
A package's own `owners` SHALL replace the top-level list for that package.
Omitted at both levels, a package SHALL have no owner proof.
An empty list or a login listed twice SHALL fail the load and name the field.

#### Scenario: The default list

- **WHEN** `.ctl/ver.yaml` sets `owners: [any-victor]` and declares packages `core` and `app` with no `owners` of their own
- **THEN** verctl expects `any-victor` among the owners of both crates

#### Scenario: A package override

- **WHEN** `.ctl/ver.yaml` sets `owners: [any-victor]`, and package `app` sets `owners: [any-victor, alice]`
- **THEN** verctl expects `any-victor` and `alice` among the owners of `app`, and only `any-victor` for every other package

#### Scenario: An empty list

- **WHEN** package `app` sets `owners: []`
- **THEN** loading `.ctl/ver.yaml` fails and names `packages[app].owners`
