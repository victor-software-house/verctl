## ADDED Requirements

### Requirement: Workspace files declare the packages

With no `packages` key, verctl SHALL infer the packages from root files only: `Cargo.toml` with `[workspace]`, `pnpm-workspace.yaml`, `package.json` `workspaces`, `go.work`, and `pyproject.toml` with `[tool.uv.workspace]`.
Members SHALL come only from the globs those files declare; no provider SHALL scan the tree.

#### Scenario: A Bun workspace

- **WHEN** the root `package.json` holds `"workspaces": ["packages/*"]` and `packages/a` and `packages/b` each hold a `package.json`
- **THEN** verctl releases `a` and `b` with no `packages` key in `.ctl/ver.yaml`

#### Scenario: A directory no workspace file lists

- **WHEN** `tools/gen/package.json` exists and no workspace glob matches `tools/gen`
- **THEN** verctl does not release `tools/gen`

### Requirement: A root with no workspace file is one package

When the root holds no workspace file, verctl SHALL release one package: the root manifest, or `VERSION` when the root holds no manifest.

#### Scenario: A repository with only a VERSION file

- **WHEN** the root holds `VERSION` and no manifest, and `.ctl/ver.yaml` has no `packages` key
- **THEN** verctl releases one package, versioned in `VERSION`

### Requirement: Every provider runs

verctl SHALL run every provider in a fixed order and sort members by path.
A root that holds two workspace kinds SHALL give the packages of both, never one merged package.
A provider that cannot read its file SHALL fail the load and name that file.

#### Scenario: A Cargo and a Bun workspace at one root

- **WHEN** the root `Cargo.toml` has `[workspace]` with member `crates/core`, and the root `package.json` has `workspaces: ["web"]`
- **THEN** verctl releases both `crates/core` and `web`

### Requirement: A packages entry overrides an inferred package

A `packages` entry whose `name` matches an inferred package SHALL override that package's fields one by one, and `remove: true` SHALL drop it.
An entry whose name nothing inferred SHALL need a `path`, and SHALL add a package.

#### Scenario: Turning one member's publisher off

- **WHEN** `web` is inferred and `.ctl/ver.yaml` declares `{name: web, publisher: none}`
- **THEN** verctl releases `web` on GitHub only and keeps its inferred `path`

#### Scenario: An entry with no inferred match and no path

- **WHEN** `.ctl/ver.yaml` declares `{name: ghost}` and no package named `ghost` is inferred
- **THEN** the load fails and names `ghost` and `path`

### Requirement: Root globs add packages

`discover` SHALL take root globs that add packages no workspace file lists.
A leading `!` SHALL exclude, `node_modules`, `.git`, and `target` SHALL always be excluded, and a match outside the repository SHALL fail the load.

#### Scenario: A directory of actions

- **WHEN** `.ctl/ver.yaml` holds `discover: [actions/*, "!actions/internal"]` and `actions/setup/VERSION` and `actions/internal/VERSION` exist
- **THEN** verctl releases `setup` and not `internal`

### Requirement: A package is named by its manifest

verctl SHALL name a package by its manifest: `package.name`, `name`, or the Go module path, and by its directory for a `VERSION` package.
Two packages with one name, or one manifest claimed twice, SHALL fail the load and name both files.
A manifest with no name SHALL fail the load and name the override that fixes it.

#### Scenario: Two members with one name

- **WHEN** `packages/a/package.json` and `packages/b/package.json` both hold `"name": "app"`
- **THEN** the load fails and names both files

### Requirement: Packages that do not publish are still versioned

A member marked `"private": true` or `publish = false` SHALL still be versioned and changelogged.

#### Scenario: A private package

- **WHEN** inferred package `internal` holds `"private": true` and `.changeset/fix.md` holds `internal: patch`
- **THEN** `verctl prepare` bumps `internal`

### Requirement: A package owns the files under its version source

A package SHALL own the directory of its version source and every file below it that no deeper package owns.
`files` SHALL replace that set with globs.

#### Scenario: A nested package

- **WHEN** packages live at `.` and `crates/core`, and `crates/core/src/lib.rs` changed
- **THEN** the change belongs to `crates/core` and not to the root package

### Requirement: check reports a changed package with no fragment

`check` SHALL report each package whose owned files changed since the merge-base with the default branch while no fragment names it.
A changed file no package owns SHALL be reported once.

#### Scenario: A change with no fragment

- **WHEN** `packages/a/index.ts` changed on the branch and no fragment names `a`
- **THEN** `verctl check` exits non-zero and names `a`

### Requirement: status explains discovery

`status` SHALL list each package with its provider, its version source, and whether it was inferred, overridden, or added, plus each provider that found nothing.

#### Scenario: Reading where a package came from

- **WHEN** `web` is inferred from `package.json` `workspaces` and overridden in `.ctl/ver.yaml`
- **THEN** `verctl status --format json` reports `web` with provider `package.json` and origin `overridden`
