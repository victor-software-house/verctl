# Design

## 1. A removed verb names its replacement

`verctl prepare` and `verctl pin` stay in the command enum, hidden from help
and from the usage spec. Each takes any arguments and exits 2, the code clap
uses for a usage error, with one line on stderr: `prepare is now version: run
verctl version` and `pin is now render: run verctl render`. Under
`--format json` the same message is the JSON error on stdout.

Alternative considered: let clap refuse the old verb as an unknown command.
It lost because the error would say only that the command does not exist,
and the operator approved the stub on 2026-11-09 (operator, 2026-11-09).

Alternative considered: keep the old verbs as aliases. It lost because two
names for one command is the parallel path the minor exists to remove.

The stubs are removed in a later minor. Public override: none; disable path:
none, since both exist only to fail.

## 2. `next` becomes `after` everywhere

The report is untagged JSON, so only the field name changes. The human view
labels the same rows `after`. No other report field is renamed.

## 3. `render` keeps `pin`'s behaviour

`render` rewrites the `pins` entries, as `pin` does today. It does not render
`.ctl/templates/`, which `version` already does. Moving templates onto
`ctl_core::served`, and making `render` the one verb that writes every served
file, is a later change. The rename comes first so that callers move once.

## 4. Config keys stay

`prepare` (the `after`, `stage`, and `stage_ignored` keys) and `pins` keep
their names in `.ctl/ver.yaml`. Renaming them belongs to the config reshape,
which the operator kept out of 0.8.0.

## 5. Specs move with the names

Eight requirements in `changelog-references`, `env-flags`, `git-access`,
`version-pr-auto-merge`, and `versioning` name `prepare` in their text or
scenarios. This change carries a MODIFIED delta for each, with the new verb
and no other change.

Three open changes ship before this one and add requirements that name the
old verb: `version-files`, `derived-version-pr-branch` (the
`version-pr-branch` capability, split from `reusable-release-workflow`), and
`version-pr-proposal`. This change
carries a MODIFIED delta for each of those requirements too, so it is
archived after them. `prepare` as an `assets` step key in
`package-asset-files` is a config key, not the verb, and stays.

`dependent-ranges`, `package-discovery`, and `release-groups` are built after
0.8.0, so their deltas already say `version`.
