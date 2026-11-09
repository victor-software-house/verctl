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

1. A stock driver is chosen by the file a package's `path` names:
   1. `Cargo.toml` and `package.json`, as today.
   2. `pyproject.toml`: `project.version`.
   3. `go.mod`: a `// vX.Y.Z` comment on the `module` line, the form Knope reads and writes. A bump to a new major version is refused unless the module path already ends in that `/vN`.
   4. `VERSION`: a file that holds the version and a newline, and nothing else.
2. A `path` that names a directory resolves to the manifest it holds, in that order, and to `<dir>/VERSION` when it holds none.
   So `packages: [{name: templates, path: .}]` versions a repository with no manifest.
3. A package with a `VERSION` file and no `publisher` releases on GitHub only (tag and Release), as `publisher: none` does today.
4. A first release starts from `0.0.0`, as every package does: verctl writes `VERSION` with `0.0.0` when `check` or `prepare` finds the file missing and the package declared.
   A repository already tagged by hand, such as templates at `v0.0.75`, commits `VERSION` at its newest tag's version once, by hand, in the adoption pull request.
5. `prepare` writes the next version into that file, the CHANGELOG heading, and nothing else for the package, so the Version PR stays the only writer.

## Capabilities

### New Capabilities

- `version-files`: which tracked file holds a package's version, chosen by file name, with `VERSION` as the fallback.

### Modified Capabilities

- `versioning`: a package's current version is read from its version file, whatever the stack.

## Impact

1. Code: `src/config.rs` (stock drivers, directory paths), `src/driver.rs` (`go.mod` and plain-file formats), `src/detect.rs`, tests.
2. Docs: the instructions, the operator skill, `README.md`, `docs/`.
3. Adopters: the templates and actions repositories each add `.ctl/ver.yaml`, `VERSION`, `CHANGELOG.md`, and the release workflow. Their owner session makes those changes; verctl ships only the support.
4. Not in this change: finding packages in a monorepo ([`package-discovery`](../package-discovery/proposal.md)), per-package tags ([`release-groups`](../release-groups/proposal.md)), internal ranges ([`dependent-ranges`](../dependent-ranges/proposal.md)), and immutable releases ([`release-immutability`](../release-immutability/proposal.md)).
