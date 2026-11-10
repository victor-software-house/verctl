# Ship cargo and bun as built-ins, with bun-release in Rust

## Motivation

Changesets publishes Bun workspaces through npm, which ships `workspace:`
ranges raw, and Bun's own lockfile, not package.json, decides the dependency
versions it publishes. bun-release covers that in TypeScript beside verctl,
and verctl's own tool pin moves by hand.

Queue row [VER-011][ver-011]. This change is one part of
[release-state-machine][rsm]; the blocks it provides are
[building-blocks][bb].

## Changes

1. Cargo and Bun are built-in providers of the Manifest, Lockfile, Artifact,
   and Registry blocks.
2. mise installs and verifies verctl, runs every step, and owns `mise.lock`.
   A repository's own tool pin moves through the built-in `mise-pin` step.
3. bun-release becomes Rust inside verctl. Bun packs. verctl's own npm
   registry client queries, publishes, exchanges OIDC tokens, and handles
   staging, trust, and provenance.

## Capabilities

### New Capabilities

- `built-in-ecosystems`: the cargo and bun providers, the `mise-pin` step,
  and the OIDC exchange.

### Modified Capabilities

None.

## Decisions for the operator

The operator approved these as decisions 6 and 7 of
[release-state-machine][rsm] on 2026-10-12.

6. mise installs, verifies, runs steps, and owns `mise.lock`; verctl writes no
   lockfile (design 3). Partly settled: verctl writes no lockfile today.
7. bun-release in Rust as option (b): Bun packs, verctl owns the registry
   protocol (design 2). Not settled by code.

## Impact

1. Code: the cargo and bun providers, and a new npm registry client.
2. bun-release retires once the bun ecosystem publishes it.

[rsm]: ../release-state-machine/proposal.md
[bb]: ../building-blocks/proposal.md
[ver-011]: ../../../tasks.yaml
