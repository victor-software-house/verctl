# Proposal

## Motivation

Some repositories build a third-party library from a pinned upstream release.
They publish the result as GitHub Release assets, so mise can install it with a `github:` tool.
verctl cannot do that today, so such a workflow falls back to `gh release` or a release action.
Three things stand in the way, read at [`39a26ee`][base]:

1. An asset tarball packs one binary (`tar czf … -C <dir> <bin>`) and is named `{bin}_{version}_{os}_{arch}.tar.gz` ([`src/assets.rs`][assets]).
2. The version and the tag come from the first package and the Changesets loop, while a vendored library carries its upstream version.
3. `assets --upload` needs a Release that already exists; only `publish` creates one ([`src/github.rs`][github]).

Queue row: [VER-080][ver-080].

## Changes

1. `.ctl/ver.yaml` gains a `vendored` section of named asset sets.
   Each set has:
   - its own `version`
   - a `tag` and an `archive` name template
   - an optional `build` argv
   - a `dir` and the `files` to pack
   - an optional `notes` file
   - `targets` that each name one runner
2. `verctl assets --vendored <name>` (`VERCTL_VENDORED`) plans that set.
   - `--build <id>` runs its build and packs the listed files.
   - `--upload` creates the set's Release when it is missing and attaches the archive and a `<archive>.sha256` file.
3. A config may declare no package when it declares a vendored set, so a repository can publish vendored assets without adopting the release loop.
4. `assets` without `--vendored` keeps its plan, archive, output, and upload byte for byte.

## Capabilities

### New Capabilities

- `vendored-assets`: declaring, building, and publishing an asset set whose version is not a package's.

### Modified Capabilities

None.

## Impact

1. Code: [`src/config.rs`][config], [`src/assets.rs`][assets], [`src/github.rs`][github], [`src/cli.rs`][cli], [`src/main.rs`][main], [`src/presentation.rs`][presentation].
2. Docs: [`src/instructions.md`][instructions], [`skills/verctl/SKILL.md`][skill], their templates under `.ctl/`, and [`README.md`][readme].
3. Callers: none change. A repository opts in by declaring `vendored`.

[base]: https://github.com/victor-software-house/verctl/tree/39a26ee
[assets]: ../../../src/assets.rs
[github]: ../../../src/github.rs
[config]: ../../../src/config.rs
[cli]: ../../../src/cli.rs
[main]: ../../../src/main.rs
[presentation]: ../../../src/presentation.rs
[instructions]: ../../../src/instructions.md
[skill]: ../../../skills/verctl/SKILL.md
[readme]: ../../../README.md
[ver-080]: ../../../tasks.yaml
