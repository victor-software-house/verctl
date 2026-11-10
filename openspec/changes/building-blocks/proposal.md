# Build a release from blocks, with steps as the extension unit

## Why

Six of six repositories restate the same lockfile refresh by hand, because
verctl works it out and only prints it. The only extension point is
`prepare.after` with `stage` globs, whose effect is unknown.

Queue row: [VER-011][ver-011]. This change is one part of
[release-state-machine][rsm]; the cargo and bun providers of its blocks are
[built-in-ecosystems][bie].

## What Changes

1. A release is built from blocks (Manifest, Lockfile, Artifact, Registry,
   Forge, Served files, Binary asset, Step). Any package can replace a block
   with a step, and any stack can declare a whole ecosystem from steps.
2. A step is a mise task or an argv with declared `writes` and, after a
   release, a `check`. Steps run at `version`, `verify`, `publish`, and
   `released`. They replace `prepare.after` and its `stage` globs.
3. No lockfile is ever written by verctl; each is written by its owner and
   read by verctl to check it.
4. Packages publish in workspace dependency order, and only the dependencies
   that count toward publishing order them.

## Capabilities

### New Capabilities

- `building-blocks`: the blocks, declared ecosystems, steps, lockfile
  ownership, and the dependency graph.

### Modified Capabilities

None.

## Decisions for the operator

The operator approved this as decision 5 of [release-state-machine][rsm] on
2026-10-12.

5. Built-ins as providers of building blocks; extensions as steps run through
   mise tasks or argv; declared ecosystems for other stacks (design 1). Not
   settled by code: drivers and publishers remain.

## Impact

1. Code: `src/config.rs`, `src/versions.rs`, `src/publish.rs`, and a new
   steps module.

[rsm]: ../release-state-machine/proposal.md
[bie]: ../built-in-ecosystems/proposal.md
[ver-011]: ../../../tasks.yaml
