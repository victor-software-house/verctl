# Design

## Context

See [proposal.md](proposal.md) for why, and [the spec](specs/version-files/spec.md) for what.

Today a package's `path` must name a manifest file.
The stock drivers are `cargo` (TOML keys) and `npm` (JSON keys), chosen from the file name, and any other file needs a `drivers` entry.
The stock publisher is also chosen from the file name: `cargo` for `Cargo.toml`, `bun` for `package.json`.
`.ctl/ver.yaml` is parsed and validated once, at `Config::parse`, and nothing downstream checks it again.

## Goals / Non-Goals

Goals: four new stock version sources and the load-time checks the spec lists, all at the parsing boundary. Directory paths, publisher inference, and changelog collisions are in [`package-resolution`](../../package-resolution/design.md).

Non-goals:

1. Finding packages without a `packages` list ([`package-discovery`](../../package-discovery/proposal.md)).
2. Tags per package or group ([`release-groups`](../../release-groups/proposal.md)).
3. A Python publisher. `pyproject.toml` packages name theirs, or `none`.
4. Rewriting a Go module path on a major bump. That is a source change a person makes.

## Decisions

### 1. Each version source is a stock driver

`pyproject.toml` is the existing TOML driver with the key `project.version`.
`go.mod`, `VERSION`, and the marker line are three new driver formats beside `toml` and `json`.
Each reads one version out of the file's text and writes it back in place.
So a repository still overrides any of them with `driver`, inline fields, or `read` and `write` commands, as it does for Cargo today.

Alternative: a generic regex writer (queue row [VER-008][ver-008]) for all three.
It lost because each format has a rule a pattern cannot state.
The rules are `go.mod`'s major-version path rule, `VERSION`'s nothing-else rule, and the marker's exactly-one-line rule.
That queue row stays open for a real fourth manifest.

### 2. The marker is opt-in with `marker: true`

A file of any type becomes a version source only when its package declares `marker: true`.
The marker line is a comment opener (`#`, `//`, or `<!--`), `version:`, a space, and the version.

Alternative: detect a marker line in any file a `path` names.
It lost because a stray `# version:` line in a file would change how verctl reads it with no declaration saying so.
Override: a `drivers` entry for that file. Disable: drop `marker: true`.

### 3. A missing VERSION file is 0.0.0

Every new package starts at `0.0.0` (the `versioning` spec), so a missing `VERSION` reads as that, and `prepare` writes the file with the first version.
`check` never writes.

Alternative: `check` or a new verb creates `VERSION` at `0.0.0`.
It lost because `check` would then change the tree it checks.
A new verb also adds an adoption step that the first Version PR already does.

## Risks / Trade-offs

1. A `pyproject.toml` using Poetry 1's `tool.poetry.version` reads as having no `project.version` → the load fails and names the key; a `keys` override (`[tool.poetry.version]`) fixes it with no new code.
2. A `go.mod` comment edited by hand to a version that conflicts with the last tag.
   `check --versions` already compares manifests to the merge-base, so the drift shows in review.
3. Byte-preserving edits are only as good as the editor.
   TOML goes through `toml_edit`, as Cargo does.
   The three text formats rewrite one span and keep every other byte.
   Each gets a round-trip test.

## Migration Plan

Additive: a repository whose packages name `Cargo.toml` or `package.json` sees no change.
Ships in the next minor, not as 0.7.x patches, because the `Driver` enum gains a variant, an API break. The slice waits with the rest of the minor backlog (operator, 2026-11-09). Every public enum that can grow is sealed in the same release.

[ver-008]: ../../../../tasks.yaml
