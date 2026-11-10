# Proposal

## Motivation

When a package in a monorepo moves to a new version, the packages that depend on it may hold a range the new version leaves.
They must be published after it.
verctl handles this only for a Cargo workspace, through Cargo.

For Bun and pnpm workspaces it does nothing.
A dependent keeps its old range.
Publish follows declaration order and ignores the dependency graph (queue row [VER-033][ver-033]).

The [research](../archive/2026-11-10-version-files/research.md) found one rule shared by pnpm 11 and Changesets.
Leave `workspace:*`, `workspace:^`, and `workspace:~` alone, because the package manager rewrites them in the packed manifest.
Touch only an explicit range.

Queue row: [VER-083][ver-083]. Folds in [VER-033][ver-033].

## Changes

1. `prepare` leaves `workspace:*`, `workspace:^`, `workspace:~`, and `catalog:` specs unchanged.
2. `prepare` rewrites an explicit internal range (`workspace:^1.2.0` or `^1.2.0`) that no longer admits the new version, keeping its prefix and operator.
   It bumps that dependent by a patch, as pnpm 11 does.
   A range the new version still satisfies is left alone.
3. `check` warns about an internal production dependency that does not use `workspace:`, so a repository can adopt pnpm's `enforceWorkspaceProtocol` rule; `dependents.require_workspace_protocol: true` turns the warning into a failure.
4. `publish` releases packages in topological order of the internal dependency graph, restricted to the packages being released, and fails on a cycle, naming it.
5. verctl never writes a lockfile; the package manager's own install refreshes it, as `prepare.after` does today.

## Capabilities

### New Capabilities

- `dependent-ranges`: how internal dependency ranges follow a new version, and the order packages publish in.

### Modified Capabilities

None.

## Impact

1. Code: a dependency graph per workspace kind, `src/prepare.rs`, `src/publish.rs`, tests with Bun and pnpm fixtures.
2. Queue row [VER-033][ver-033] closes with this change.
3. Depends on [`package-discovery`](../package-discovery/proposal.md).

[ver-033]: ../../../tasks.yaml
[ver-083]: ../../../tasks.yaml
