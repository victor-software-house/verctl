# Proposal

## Why

Every package verctl releases is listed by hand in `.ctl/ver.yaml`.
A Cargo workspace is the one exception: its members come from `cargo metadata`.
A Bun or pnpm workspace is not read.
Each member would have to be listed again beside the `workspaces` field that already lists it, and the two lists drift.

Every monorepo tool read for the [research](../version-files/research.md) declares its members once, at the root.
Turborepo and manypkg read the package manager's own declaration.
mise reads root globs and has deprecated scanning every directory.
Each names a package by its manifest and fails on a duplicate name.

Queue row: [VER-085][ver-085], under the research in [VER-083][ver-083].
The files each package owns, and the `check` that uses them, are in [`package-ownership`](../package-ownership/proposal.md), split from this change at that boundary.

## What Changes

1. With no `packages` key, verctl infers the packages from the repository root, the way mise's workspace providers do (operator, 2026-11-09).
   One provider per stack reads only root files:
   - `Cargo.toml` with `[workspace]` (through `cargo metadata`, as today);
   - `pnpm-workspace.yaml`;
   - `package.json` `workspaces` (array or `packages` object, for Bun, npm, and Yarn);
   - `go.work`;
   - `pyproject.toml` with `[tool.uv.workspace]`.

   A root with no workspace file is one package: its root manifest, or `VERSION` when it has none.
   No provider scans the tree; members come only from the globs a workspace file declares.
2. Every provider runs. A root that holds both `Cargo.toml` and `package.json` gives two packages, never one fused package.
   Providers run in a fixed order, members sort by path, and a provider that cannot read its file reports itself broken without hiding the others.
3. A `packages` entry is keyed by the inferred name and overrides that package field by field; `remove: true` drops it.
   An entry whose name nothing inferred needs a `path` and adds a package, so a repository can still list every package by hand.
4. Root globs add packages no workspace file lists, such as a directory of `VERSION` packages: `discover: [actions/*, "!actions/internal"]`.
5. Glob rules follow Turborepo and mise:
   - a leading `!` excludes;
   - `node_modules`, `.git`, and `target` are always excluded;
   - each match is resolved to its real path;
   - a match outside the repository fails the load.
6. A package is named by the manifest (`package.name`, `name`, the Go module path), and by its directory for a `VERSION` package, so moving a manifest keeps its name.
   A duplicate name, or one manifest claimed twice, fails the load and names both files.
   A manifest with no name fails and names the override that fixes it.
7. A member that does not publish (`"private": true`, `publish = false`) is still versioned, as today.
8. `status` reports discovery:
   - each package with its provider, its source file, and whether it was inferred, overridden, or added;
   - the providers that found nothing;
   - every path a rule skipped.

## Capabilities

### New Capabilities

- `package-discovery`: how verctl finds a repository's packages from workspace files and root globs, and names them.

### Modified Capabilities

- `release-plan`: the plan covers discovered packages as well as listed ones.

## Impact

1. Code: `src/config.rs`, `src/members.rs` (generalized from Cargo), a workspace reader per stack, tests with a fixture per workspace kind.
2. A repository that lists every package by hand keeps working unchanged: an explicit entry overrides inference.
3. Depends on [`version-files`](../version-files/proposal.md) for the version file of each member.

[ver-083]: ../../../tasks.yaml
[ver-085]: ../../../tasks.yaml
