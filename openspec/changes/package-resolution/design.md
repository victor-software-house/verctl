# Design

## Context

See [proposal.md](proposal.md) for why, and [the spec](specs/package-resolution/spec.md) for what.
Today a package's `path` must name a manifest file.
The stock publisher is chosen from the file name: `cargo` for `Cargo.toml`, `bun` for `package.json`.
`.ctl/ver.yaml` is parsed and validated once, at `Config::parse`, and nothing downstream checks it again.
The version sources a path can resolve to are in [`version-files`](../archive/2026-11-10-version-files/design.md).

## Decisions

### 1. A directory resolves once, at parse time

`Config::parse` replaces a directory `path` with the file it resolves to, so every later step sees a file, as it does today.
Two manifests in one directory fail there, with both names.

Alternative: a fixed priority order (`Cargo.toml` before `package.json`).
It lost because a directory holding both is a Rust crate with JavaScript bindings or the other way round.
Guessing which one carries the release version would be wrong half the time.
Override: name the file.

### 2. Publisher inference follows the version source

`VERSION`, a marker line, and `go.mod` infer `publisher: none`.
`pyproject.toml` infers nothing, so its load fails and names `publisher` until a Python publisher ships.

Alternative: infer `none` for Python too.
It lost because a Python package that releases without uploading to PyPI would look released and not be.
Override: `publisher:` on the package.

### 3. Changelog collisions fail the load

Two packages resolving to one changelog file fail `Config::parse`, naming both and `changelog`.
This is general, not marker-specific, so it also catches two manifests that share a directory through custom drivers.

Alternative: a per-file default such as `ci.CHANGELOG.md` for marker packages.
It lost because it invents a file-name convention no other tool reads, and the explicit `changelog` key already exists.

## Migration Plan

Additive: a repository whose packages name `Cargo.toml` or `package.json` sees no change.
It ships in the next minor with [`version-files`](../archive/2026-11-10-version-files/design.md), whose version sources it resolves.
