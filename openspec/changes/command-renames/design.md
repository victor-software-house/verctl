# Design

## 1. A removed verb names its replacement

`verctl prepare`, `pin`, `publish`, and `assets` stay as hidden commands,
out of help. The usage spec marks them `hide`, so mise leaves them out of
its help and completion. Each takes any arguments and exits 2, the code
clap uses for a usage error, with one line on stderr, such as `prepare is
now version: run verctl version` and `assets is now release asset: run
verctl release asset`. Under
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

## 3. `render` writes every served file

`render` rewrites the `pins` entries, as `pin` did, then renders
`.ctl/templates/` at the versions on HEAD. The rendering half ships in
[served-file-check](../served-file-check/design.md), which makes `check`
fail on a served file that differs and needs one command that fixes it.
Moving templates onto `ctl_core::served` is still a later change.

## 4. Config keys stay

`prepare` (the `after`, `stage`, and `stage_ignored` keys) and `pins` keep
their names in `.ctl/ver.yaml`. Renaming them belongs to the config reshape,
which the operator kept out of 0.8.0.

## 5. `release` and `release asset`

`release` keeps every flag `publish` took except `--plan`. That flag stays
hidden on `release`, and any value but false fails the parse with `--plan
is now status --github-output: run verctl status --github-output`.

`release asset <target>` takes its target as a positional with
`VERCTL_TARGET`, since a positional gets no `EnvPrefix` binding. It builds
the tarball and uploads it unless `--dry-run`. With `--vendored <name>` it
builds and uploads that set's archive instead. One verb replaces the
`--build` and `--upload` pair, so a workflow step cannot upload what it did
not build.

## 6. `status` holds the plan

`status` reports the fragments, the asset plan, and the version commits
with an incomplete release, under `unreleased`, oldest first.
`--skip-assets` leaves missing assets out of that check, as it did on
`publish --plan`. A missing `.ctl/ver.yaml` fails `status` as it fails
every other verb that reads the config.

Finding unreleased commits asks GitHub, so it needs `GITHUB_TOKEN` or
`GH_TOKEN`. Without one, `status` still succeeds: JSON holds
`"unreleased": null` and `unreleased_unchecked` holds the reason, and the
human view reads `unreleased  not checked (no GITHUB_TOKEN or GH_TOKEN)`.
An empty list means checked and none, so the two never read alike. A
check that fails without `--github-output` is reported the same way, with
the error as the reason. Such a failure is a shallow clone that reaches its
boundary, no GitHub `origin`, or no network. `status` is a local view, so a
lookup it only adds must not fail it. The human view shows the error's
top-level message, and JSON keeps its whole chain. Under `--github-output`
the error fails the run.

Without `--github-output`, `status` changes nothing in the clone: no fetch
and no unshallow, so a shallow clone checks only the history it has. With
`--github-output <file>`, it fetches the history the plan needs, requires a
token, and writes the outputs under the names `assets --github-output` and
`publish --plan` used, so `release.yml` changes only the verb it calls.

Alternative considered: keep `publish --plan` as `release --plan` until
0.9.0. It lost because 0.9.0's plan job runs `status --github-output`, and
moving it then would be a second break.

## 7. Specs move with the names

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

For `publish`, this change carries a MODIFIED delta for each requirement in
`env-flags`, `presentation`, `release-only`, `release-plan`,
`release-preconditions`, and `release-tag-order` that names the verb. Other
open changes that name `publish`, `publish --plan`, or `assets` keep their
text; each is rewritten to the new names when it is archived.
