# Make the release an explicit state machine

## Motivation

verctl has no model of a release. Each command rebuilds the state it needs
from indirect signals, and each gap has grown a workaround. An inventory of
this repository and its five consumers found 27 of them, and 13 ledger rows
that propose more of the same.

One measured example: the stack is guessed from the manifest file name in
four places. The tag shape is set in three, and the default branch comes from
five inputs. Each part below names its own.

Queue row: [VER-011][ver-011]. This change replaces the plan held on
`feat/011-refresh-lockfile` and folds in its two approved intents. The version
step refreshes lockfiles itself, and the report field `next` becomes `after`.

## Changes

This change holds what every part shares: the goals, `.ctl/ver.yaml`, the
decisions, the workaround inventory, and the migration. Each part is its own
change with its own proposal. The parts are [derived-release-state][drs],
[release-completion][rc], [`version-pr-identity`][vpi],
[version-pr-content][vpc], [building-blocks][bb], and
[built-in-ecosystems][bie].

1. **BREAKING:** `.ctl/ver.yaml` keeps one place per concern and refuses
   unknown keys at every level.
   1. These keys go: `drivers`, `publishers`, `prepare`, `pins`, `patterns`,
      and the per-package driver and publisher keys.
   2. These keys replace them: `branch`, `registries`, `ecosystems`, `steps`,
      `at`, `proposal`, `tags`, and `packages[].assets`.
   3. Partly shipped: unknown keys are refused, and every removed key is
      still read ([audit](audit.md)).
2. All breaking changes ship in one minor, 0.8.0, held with the rest of the
   minor backlog. Later work ships as patches that add without breaking.
   0.4.0 to 0.7.4 shipped before this change, and the parts of it they
   carried are listed in [audit.md](audit.md).

## Capabilities

### New Capabilities

- `release-config`: one place per concern in `.ctl/ver.yaml`, refusal of
  unknown and removed keys, and the defaults.

### Modified Capabilities

None. [`versioning`](../../specs/versioning/spec.md) keeps its first-release
rule.

## Decisions for the operator

Agreed already: templates only (design decision 14) and command renames
(design decision 15). Each decision has a recommendation; the design gives
the alternatives and trade-offs. The operator approved all eight as
recommended on 2026-10-12 (note on [VER-011][ver-011]) and confirms the 0.8.0 scope after
the [audit](audit.md).

Decisions 1 to 7 are stated in the part each belongs to. Each decision carries a note on
what shipped code settled.

8. Ship 0.4.0 from an integration branch `next`, then patches (design 4).
   Contradicted: 0.4.0 to 0.7.4 shipped from `main`, no `next` exists, and
   the 0.8.0 minor is held on a stack instead.

## Impact

1. Code: most of `src/`. `templates.rs` moves onto `ctl_core::served`, agreed
   with the session that owns [CTC-008][ctc-008].
2. Consumers: qctl, forkctl, ctl-core, unixd, and yamled change config and
   workflows once, in the 0.8.0 window. bun-release retires once the bun
   ecosystem publishes it.
3. Docs: README, the instructions, the operator skill, AGENTS.md, and
   `docs/served-files.md` are rewritten for 0.8.0.

[drs]: ../derived-release-state/proposal.md
[rc]: ../release-completion/proposal.md
[vpi]: ../version-pr-identity/proposal.md
[vpc]: ../archive/2026-11-10-version-pr-content/proposal.md
[bb]: ../building-blocks/proposal.md
[bie]: ../built-in-ecosystems/proposal.md
[ver-011]: ../../../tasks.yaml
[ctc-008]: https://github.com/victor-software-house/ctl-core/blob/8a4095fbe4d30b5ea067682f13478be0ba21fbac/tasks.yaml
