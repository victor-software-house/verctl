## ADDED Requirements

### Requirement: Workspace and catalog specs stay as written

`version` SHALL leave `workspace:*`, `workspace:^`, `workspace:~`, and `catalog:` dependency specs unchanged, because the package manager rewrites them when it packs.

#### Scenario: A workspace star

- **WHEN** `app` depends on `"core": "workspace:*"` and `core` moves to `2.0.0`
- **THEN** `verctl version` leaves `"core": "workspace:*"` in `app/package.json`

### Requirement: An explicit range follows the new version

`version` SHALL rewrite an explicit internal range that no longer admits the new version, keeping its prefix and operator.
`version` SHALL bump that dependent by a patch.
A range the new version still satisfies SHALL stay as written.

#### Scenario: A caret range left behind

- **WHEN** `app` depends on `"core": "workspace:^1.2.0"` and `core` moves to `2.0.0`
- **THEN** `verctl version` writes `"core": "workspace:^2.0.0"` and bumps `app` by a patch

#### Scenario: A range that still admits the version

- **WHEN** `app` depends on `"core": "^1.2.0"` and `core` moves to `1.3.0`
- **THEN** `verctl version` leaves the range and does not bump `app`

### Requirement: check asks for the workspace protocol

`check` SHALL warn about an internal production dependency that does not use `workspace:`.
`dependents.require_workspace_protocol: true` SHALL make that warning a failure.

#### Scenario: A plain range under the strict setting

- **WHEN** `dependents.require_workspace_protocol` is `true` and `app` depends on `"core": "^1.2.0"`
- **THEN** `verctl check` exits non-zero and names `app` and `core`

### Requirement: publish follows the dependency graph

`publish` SHALL release packages in topological order of the internal dependency graph, limited to the packages being released.
`publish` SHALL fail on a cycle, naming every package in it.

#### Scenario: A dependent released with a dependency

- **WHEN** `app` depends on `core`, both release, and `app` is declared first
- **THEN** `verctl publish` uploads `core` before `app`

### Requirement: verctl never writes a lockfile

verctl SHALL never edit a lockfile; the bump's follow-up names the package manager's own refresh.

#### Scenario: A pnpm workspace

- **WHEN** a dependent's range moves in a pnpm workspace
- **THEN** `pnpm-lock.yaml` is unchanged by `verctl version` and the report lists `pnpm install` under `after`
