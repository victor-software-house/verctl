# Proposal

## Motivation

A repository with no manifest cannot release through verctl at all.
The templates repository has been tagged by hand 75 times, `v0.0.1` to `v0.0.75`, two of the tags lightweight, with no Release.

[`version-files`](../version-files/proposal.md) gives such a repository a version source, a `VERSION` file or a marker line.
A package still has to name that file, needs a publisher where nothing uploads, and single-file packages in one directory would share a changelog.

Queue row: [VER-083][ver-083]. This change was split from `version-files` at one boundary of its work.
That change reads and writes each version source.
This one resolves which package uses which file.

## Changes

1. A `path` that names a directory resolves to the one manifest it holds, and to `<dir>/VERSION` when it holds none.
   So `packages: [{name: templates, path: .}]` versions a repository with no manifest. A directory with two manifests fails the load and asks for a file path.
2. A package whose version lives in `VERSION`, a marker line, or `go.mod`, with no `publisher`, releases on GitHub only (tag and Release), as `publisher: none` does today. Go modules need no upload: the Go proxy reads the tag.
   A `pyproject.toml` package must name its publisher until verctl ships a Python one.
3. Two packages that resolve to the same changelog fail the load, which matters once single-file packages share a directory.

## Capabilities

### New Capabilities

- `package-resolution`: what a directory `path` resolves to, which packages release on GitHub only, and one changelog per package.

## Impact

1. Code: `src/config.rs` (directory paths, publisher inference, changelog collisions), tests.
2. Docs: the instructions, the operator skill, `README.md`, `docs/`.
3. Adopters: the templates and actions repositories declare `path: .` or a directory per package.

[ver-083]: ../../../tasks.yaml
