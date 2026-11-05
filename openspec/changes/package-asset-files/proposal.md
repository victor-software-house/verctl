# Proposal

## Why

A package asset tarball holds exactly one file.
`assets --build` runs `tar czf <asset> -C <parent of binary> <bin>` ([`src/assets.rs`][assets]), so a package whose release is a daemon, its client, a systemd unit, and a licence ships a tarball with only the first of the four.
Probed on verctl 0.7.2 with a `publisher: none` package and a `linux-x64` target: the tarball listed one entry.
Vendored sets already pack several files from a directory (`vendored.<name>.dir` and `files`); package assets cannot.

Queue row: VER-081.

## What Changes

1. `assets` in `.ctl/ver.yaml` gains optional `dir` and `files`, with the placeholders `binary` takes: `{bin}`, `{version}`, `{id}`, `{os}`, `{arch}`, `{triple}`.
2. With them, `assets --build <id>` runs `prepare` and `build` as today, checks that each listed path exists under `dir`, then packs `tar czf <asset> -C <dir> <files…>`.
3. The load fails when `dir` comes without `files`, `files` without `dir`, `binary` beside them, or a path leaves the repository.
4. Without them, the plan, the tarball, and its single entry stay byte for byte as before.

## Capabilities

### New Capabilities

- `asset-contents`: what a package asset tarball holds.

### Modified Capabilities

None.

## Impact

1. Code: [`src/config.rs`][config], [`src/assets.rs`][assets].
2. Docs: [`src/instructions.md`][instructions], [`skills/verctl/SKILL.md`][skill], their templates under `.ctl/`, [`README.md`][readme], and [`examples/ver.yaml`][example].
3. Callers: none change. A repository opts in by declaring `dir` and `files`.

[assets]: ../../../src/assets.rs
[config]: ../../../src/config.rs
[instructions]: ../../../src/instructions.md
[skill]: ../../../skills/verctl/SKILL.md
[readme]: ../../../README.md
[example]: ../../../examples/ver.yaml
