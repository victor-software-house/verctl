# Proposal

## Motivation

Every `assets` target packs the same `assets.files` list.  
A repository whose platforms ship different files cannot release them.  
One repository packs a benchmark tool and a proof tool on `linux-x64`, and only the proof tool on `darwin-arm64`, because the benchmark builds only on Linux.  
With both tools in the shared list, `release asset darwin-arm64` fails: the benchmark tool is missing after the build.

Queue row: [VER-107][ver-107].

## Changes

1. A target under `assets` may declare `files`, with the placeholders and repository checks that `assets.files` takes.
2. A target's `files` replaces `assets.files` for that target alone, and `release asset <target>` packs and checks exactly that list from `assets.dir`.
3. A target without `files` packs the shared list, as before.
4. The load fails, naming the target, when its `files` comes without `assets.dir`, is empty, or sits beside `assets.binary`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `asset-contents`: what a package asset tarball holds, from [package-asset-files][package-asset-files], gains a per-target list.

## Impact

1. Code: [`src/config.rs`][config], [`src/assets.rs`][assets].
2. Docs: [`src/instructions.md`][instructions], [`skills/verctl/SKILL.md`][skill], their templates under `.ctl/`, [`README.md`][readme], and [`examples/ver.yaml`][example].
3. Callers: none change.  
   A repository opts in when a target declares `files`.

[assets]: ../../../src/assets.rs
[config]: ../../../src/config.rs
[instructions]: ../../../src/instructions.md
[skill]: ../../../skills/verctl/SKILL.md
[readme]: ../../../README.md
[example]: ../../../examples/ver.yaml
[package-asset-files]: ../package-asset-files/proposal.md
[ver-107]: ../../../tasks.yaml
