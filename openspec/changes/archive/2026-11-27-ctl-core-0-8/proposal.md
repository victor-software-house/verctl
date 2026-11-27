# Proposal

## Motivation

verctl pins ctl-core 0.7.3, and ctl-core 0.8.0-rc.2 is published ([changelog][changelog]).
The ctl-core ledger row [CTC-043][ctl-tasks] lists, among its acceptance items, that each ctl CLI takes a command's arguments from an input document, with a golden test, and that needs 0.8.
The 0.8 releases also break the output API verctl calls: `OutputFormat` is gone, `View::new` takes a format value, and `OutputArgs` gains `--json` and `--stream`.
The queue row is [VER-147](../../../../tasks.yaml).

## Changes

1. Pin ctl-core `=0.8.0-rc.2`, with the `input-document` and `input-json` features, and migrate every 0.8 break by the changelog's migration notes.
2. Every command takes `--input <FILE>` and `--input-format <NAME>`, which ctl-core's `App` adds, read from `VERCTL_INPUT` and `VERCTL_INPUT_FORMAT` too.
3. Output that ctl-core 0.8 changed changes in verctl the same way: piped output defaults to `tsv`, a JSON failure goes to stderr, a parse error has no `error:` prefix, and tables stop at their column floor.
4. The skill and the installed instructions describe the input document and the piped default.

## Capabilities

### New Capabilities

- `input-document`: verctl commands read their arguments from a document.

### Modified Capabilities

- `presentation`: the piped default format and where a JSON failure goes.

## Impact

1. Code: `Cargo.toml`, `Cargo.lock`, `src/presentation.rs`, `src/cli.rs`, the tests and their snapshots.
2. Docs: `skills/verctl/SKILL.md`, `src/instructions.md` and their templates.
3. Release: a `verctl: patch!` changeset, since output a script reads changes.

[changelog]: https://github.com/victor-software-house/ctl-core/blob/4453827c9771269b450b70a18ac8fe2a4cffd4c6/CHANGELOG.md
[ctl-tasks]: https://github.com/victor-software-house/ctl-core/blob/4453827c9771269b450b70a18ac8fe2a4cffd4c6/tasks.yaml
