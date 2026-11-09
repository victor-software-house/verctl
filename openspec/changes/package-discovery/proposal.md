# Proposal

## Why

Every package verctl releases is listed by hand in `.ctl/ver.yaml`.
A Cargo workspace is the one exception: its members come from `cargo metadata`.
A Bun or pnpm workspace is not read, so each member would have to be listed again beside the `workspaces` field that already lists it, and the two lists drift.

Every monorepo tool read for the [research](../version-files/research.md) declares its members once, at the root: Turborepo and manypkg read the package manager's own declaration, and mise reads root globs and has deprecated scanning every directory.
Each names a package by its manifest and fails on a duplicate name.

Queue row: VER-083.

## What Changes

1. A package entry may name a workspace instead of one manifest: `packages: [{workspace: package.json}]`, `{workspace: pnpm-workspace.yaml}`, `{workspace: Cargo.toml}`, `{workspace: go.work}`, or `{workspace: pyproject.toml}` for a uv workspace.
   verctl reads the members from that file with the stack's own rules: `cargo metadata` for Cargo, as today; the globs of `workspaces` (array or `packages` object) for Bun and npm; `packages` for pnpm; `use` for `go.work`; `tool.uv.workspace.members` for uv.
2. A package entry may also name globs for packages no workspace file lists, such as a directory of `VERSION` packages: `packages: [{glob: [actions/*, "!actions/internal"]}]`.
3. Glob rules follow Turborepo and mise: a leading `!` excludes, `node_modules`, `.git`, and `target` are always excluded, each match is resolved to its real path, and a match outside the repository fails the load.
4. A member is named by its manifest name (`package.name`, `name`, the module path's last element for Go), and by its directory for a `VERSION` package.
   A duplicate name, or one manifest claimed twice, fails the load and names both paths.
5. A member that does not publish (`"private": true`, `publish = false`) is still versioned, as today.
6. Per-member settings, such as `publisher` or `first_minor`, go in an entry that names the member: `packages: [{workspace: package.json}, {name: "@scope/cli", publisher: bun}]`.
7. `status` lists every discovered package with the file it came from.

## Capabilities

### New Capabilities

- `package-discovery`: how verctl finds a repository's packages from workspace files and root globs, and names them.

### Modified Capabilities

- `release-plan`: the plan covers discovered packages as well as listed ones.

## Impact

1. Code: `src/config.rs`, `src/members.rs` (generalized from Cargo), a workspace reader per stack, tests with a fixture per workspace kind.
2. A repository that lists every package by hand keeps working unchanged.
3. Depends on [`version-files`](../version-files/proposal.md) for the version file of each member.
