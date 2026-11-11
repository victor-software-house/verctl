# Publish options belong to the package's publisher

## Motivation

The 0.8.0 release compiled verctl 7 times in one run ([run 42917622310][run]).
One of the 7 is the verify build inside `cargo publish`, 1m17s of an 86 s step.
The merge queue had already built the packed crate on the same commit.

Removing it needs one flag, `--no-verify`, and `.ctl/ver.yaml` has no place for a flag.
The only way to add one is `publish: [argv]`, which replaces the whole upload command.
A replaced command turns off three things the built-in cargo upload does:

1. the crates.io index lookup that decides whether a version is published;
2. the tag order that depends on that lookup;
3. member-by-member publishing of a workspace.

The config also asks for what the manifest already states.
15 of the 19 repositories that call `release.yml` write `publisher: none`, and their comments say the crate also sets `publish = false` (GitHub code search, 2026-11-10).

Queue row: [VER-120][ver-120].
The asset jobs' builds are the other 3 of the 7 that can go; they stay with [VER-118][ver-118] and [VER-119][ver-119].

## Changes

1. A package's `publish` takes a mapping of options for the publisher the package resolves to.
   The publisher keeps its upload command, its registry check, and its workspace handling.
   A list under `publish` still replaces the whole command, as today.
2. The built-in cargo publisher has one option, `verify`.
   `verify: false` adds `--no-verify` to its upload, for one crate and for each workspace member.
3. An option the package's publisher does not have fails the load.
   The error names the package, the publisher, and the options that publisher has.
4. A package whose manifest says it does not publish releases on GitHub only, with nothing written in `.ctl/ver.yaml`:
   a `Cargo.toml` with `publish = false` or `publish = []`, and a `package.json` with `"private": true`.
5. The plan shows the result: `via` reads `cargo, no verify` for a crate that skips the verify build.
6. verctl's own package sets `publish: {verify: false}`.

Nothing is removed.
`publishers`, `publisher`, and `publish` as a list keep working until the breaking minor that [release-state-machine][rsm] plans.

## Capabilities

### New Capabilities

- `publish-options`: the options a package may set for its publisher, how they are checked, and what each does.

### Modified Capabilities

- `release-only`: a manifest that opts out of publishing releases the package on GitHub only.
- `workspace-publish`: each member's upload carries the package's cargo options.

## Impact

1. Code: `src/config.rs` (the `publish` field), `src/publisher.rs` (one resolved publisher, cargo options), `src/pull_request.rs` (the manifest opt-out moves to where the publisher resolves), tests.
2. Bundle: `verctl instructions`, the operator skill, `README.md`, `examples/ver.yaml`, `AGENTS.md`, through their templates.
3. Callers: none must change. The 15 that write `publisher: none` beside `publish = false` may drop the line.
4. Release: a patch. Every change is an addition.

[run]: https://github.com/victor-software-house/verctl/actions/runs/42917622310
[rsm]: ../release-state-machine/proposal.md
[ver-118]: ../../../tasks.yaml
[ver-119]: ../../../tasks.yaml
[ver-120]: ../../../tasks.yaml
