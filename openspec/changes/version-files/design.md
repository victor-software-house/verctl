# Design

## Context

See [proposal.md](proposal.md) for why, and [the spec](specs/version-files/spec.md) for what.
Today a package's `path` must name a manifest file.
The stock drivers are `cargo` (TOML keys) and `npm` (JSON keys), chosen from the file name, and any other file needs a `drivers` entry.
The stock publisher is also chosen from the file name: `cargo` for `Cargo.toml`, `bun` for `package.json`.
`.ctl/ver.yaml` is parsed and validated once, at `Config::parse`, and nothing downstream checks it again.

## Goals / Non-Goals

Goals: four new stock version sources, directory paths, and the load-time checks the spec lists, all at the parsing boundary.

Non-goals:

1. Finding packages without a `packages` list ([`package-discovery`](../package-discovery/proposal.md)).
2. Tags per package or group ([`release-groups`](../release-groups/proposal.md)).
3. A Python publisher. `pyproject.toml` packages name theirs, or `none`.
4. Rewriting a Go module path on a major bump. That is a source change a person makes.

## Decisions

### 1. Each version source is a stock driver, not a new mechanism

`pyproject.toml` is the existing TOML driver with the key `project.version`.
`go.mod`, `VERSION`, and the marker line are three new driver formats beside `toml` and `json`: each reads one version out of the file's text and writes it back in place.
So a repository still overrides any of them with `driver`, inline fields, or `read` and `write` commands, as it does for Cargo today.

Alternative: a generic regex writer (VER-008) for all three.
It lost because each format has a rule a pattern cannot state: `go.mod`'s major-version path rule, `VERSION`'s nothing-else rule, and the marker's exactly-one-line rule.
VER-008 stays open for a real fourth manifest.

### 2. The marker is opt-in with `marker: true`

A file of any type becomes a version source only when its package says `marker: true`.
The marker line is a comment opener (`#`, `//`, or `<!--`), `version:`, a space, and the version.

Alternative: detect a marker line in any file a `path` names.
It lost because a stray `# version:` line in a file would change how verctl reads it with no declaration saying so.
Override: a `drivers` entry for that file. Disable: drop `marker: true`.

### 3. A directory resolves once, at parse time

`Config::parse` replaces a directory `path` with the file it resolves to, so every later step sees a file, as it does today.
Two manifests in one directory fail there, with both names.

Alternative: a fixed priority order (`Cargo.toml` before `package.json`).
It lost because a directory holding both is a Rust crate with JavaScript bindings or the other way round, and guessing which one carries the release version would be wrong half the time.
Override: name the file.

### 4. A missing VERSION file is 0.0.0

Every new package starts at `0.0.0` (the `versioning` spec), so a missing `VERSION` reads as that, and `prepare` writes the file with the first version.
`check` never writes.

Alternative: `check` or a new verb creates `VERSION` at `0.0.0`.
It lost because `check` would then change the tree it checks, and a new verb adds a step to adoption that the first Version PR already does.

### 5. Publisher inference follows the version source

`VERSION`, a marker line, and `go.mod` infer `publisher: none`.
`pyproject.toml` infers nothing, so its load fails and names `publisher` until a Python publisher ships.

Alternative: infer `none` for Python too.
It lost because a Python package that releases without uploading to PyPI would look released and not be.
Override: `publisher:` on the package.

### 6. Changelog collisions fail the load

Two packages resolving to one changelog file fail `Config::parse`, naming both and `changelog`.
This is general, not marker-specific, so it also catches two manifests that share a directory through custom drivers.

Alternative: a per-file default such as `ci.CHANGELOG.md` for marker packages.
It lost because it invents a file-name convention no other tool reads, and the explicit `changelog` key already exists.

## Risks / Trade-offs

1. A `pyproject.toml` using Poetry 1's `tool.poetry.version` reads as having no `project.version` → the load fails and names the key; a `keys` override (`[tool.poetry.version]`) fixes it with no new code.
2. A `go.mod` comment edited by hand to a version that conflicts with the last tag → `check --versions` already compares manifests to the merge-base, so the drift shows in review.
3. Byte-preserving edits are only as good as the editor → TOML goes through `toml_edit`, as Cargo does; the three text formats rewrite one span and keep every other byte, and each gets a round-trip test.

## Migration Plan

Additive: a repository whose packages name `Cargo.toml` or `package.json` sees no change.
Ships in the next minor, not as 0.7.x patches, because the `Driver` enum gains a variant, an API break. The slice waits with the rest of the minor backlog (operator, 2026-11-09). Every public enum that can grow is sealed in the same release.
