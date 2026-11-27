# Design

## Context

ctl-core 0.8 replaces the closed output enum with an open set of formats, adds `--json` and `--stream` to the shared output flags, and adds the input document behind `input-document` ([changelog][changelog]).
verctl calls `View::new(OutputFormat::…)` in its presentation tests and `is_json()` in a parser test.

## Decisions

### 1. Take ctl-core's piped default

Piped output becomes `tsv`, as ctl-core 0.8 makes it for every CLI.
A script that wants the old table passes `--format pretty` or sets `VERCTL_FORMAT=pretty`.

Alternative considered: keep `pretty` piped through `Formats::builtin().piped_default(FormatName::PRETTY)`.
It lost because ctl-core chose `tsv` so that a script reads the columns a person sees, and one CLI that differs breaks that rule.

### 2. Every command takes the document

`App` adds `--input` and `--input-format` to every command once `input-document` is on, and verctl removes them from none.
The golden test drives `status`, whose `--dir` and `--config` a document can set.

Alternative considered: `App::without_input` on every command but `status`.
It lost because the flags cost nothing per command, and one rule for every command is simpler to document.

### 3. Conflicts drop the losing key

verctl keeps ctl-core's default, where the command line, then the environment, then the document wins, and a losing key is dropped.
`App::input_conflicts(InputConflicts::Fail)` is the override verctl does not take.

### 4. A breaking patch

The changeset is `verctl: patch!`: verctl is in its `rc` train, and piped output and JSON failures change what a script reads.

## Risks

1. A caller that parsed piped pretty output breaks.
   Every workflow and task in this repository writes `--github-output` or JSON, so none reads piped tables.

[changelog]: https://github.com/victor-software-house/ctl-core/blob/4453827c9771269b450b70a18ac8fe2a4cffd4c6/CHANGELOG.md
