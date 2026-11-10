# Proposal

## Why

verctl versions only a package whose manifest it knows: `Cargo.toml`, `package.json`, or a file a repository describes with its own driver.
A repository with no manifest cannot release through verctl at all.
The templates repository has been tagged by hand 75 times, `v0.0.1` to `v0.0.75`, two of the tags lightweight, with no Release.
The actions repository has never been tagged, and five consumers sampled on 2026-11-09 pinned four different commits of it.
Go modules have the same gap: `go.mod` holds no version.

The operator ruled on 2026-11-09 that a version always lives in a tracked file, and that a `VERSION` file is the fallback when nothing else declares one.
The [research](research.md) shows the tools that version many stacks, Knope and Nx, keep each version in the package's own file.

Queue row: VER-083.

## What Changes

The operator chose all five version sources for the first slice (2026-11-09).

1. A stock driver is chosen by the file a package's `path` names:
   1. `Cargo.toml` and `package.json`, as today.
   2. `pyproject.toml`: `project.version`. A `project.dynamic` that lists `version` fails the load, because a build backend owns that version.
   3. `go.mod`: a `// vX.Y.Z` comment on the `module` line, the form Knope reads and writes. A bump to major 2 or more is refused unless the module path already ends in that `/vN`, the rule the Go toolchain enforces.
   4. `VERSION`: a file that holds the version and a newline, and nothing else.
   5. Any other text file, with `marker: true`: the version sits on one marker line, `# version: X.Y.Z`, in the file's own comment syntax.
      It serves a component that is a single file, such as a reusable workflow under `.github/workflows/`, where no other file can sit beside it.
2. A `path` that names a directory resolves to the one manifest it holds, and to `<dir>/VERSION` when it holds none.
   So `packages: [{name: templates, path: .}]` versions a repository with no manifest. A directory with two manifests fails the load and asks for a file path.
3. A package whose version lives in `VERSION`, a marker line, or `go.mod`, with no `publisher`, releases on GitHub only (tag and Release), as `publisher: none` does today. Go modules need no upload: the Go proxy reads the tag.
   A `pyproject.toml` package must name its publisher until verctl ships a Python one.
4. A missing `VERSION` file reads as `0.0.0`, as a new package does everywhere, and `prepare` creates it; `check` never writes it.
   A repository already tagged by hand, such as templates at `v0.0.75`, commits `VERSION` at its newest tag's version once, by hand, in the adoption pull request.
5. `prepare` writes the next version into that file, the CHANGELOG heading, and nothing else for the package, so the Version PR stays the only writer.
6. Two packages that resolve to the same changelog fail the load, which matters once single-file packages share a directory.

## Capabilities

### New Capabilities

- `version-files`: which tracked file holds a package's version, chosen by file name, with `VERSION` as the fallback.

## Impact

1. Code: `src/config.rs` (stock drivers, directory paths), `src/driver.rs` (`go.mod` and plain-file formats), `src/detect.rs`, tests.
2. Docs: the instructions, the operator skill, `README.md`, `docs/`.
3. Adopters: the templates and actions repositories each add `.ctl/ver.yaml`, `VERSION`, `CHANGELOG.md`, and the release workflow. Their owner session makes those changes; verctl ships only the support.
4. Not in this change: finding packages in a monorepo ([`package-discovery`](../package-discovery/proposal.md)), per-package tags ([`release-groups`](../release-groups/proposal.md)), internal ranges ([`dependent-ranges`](../dependent-ranges/proposal.md)), and immutable releases ([`release-immutability`](../release-immutability/proposal.md)).
