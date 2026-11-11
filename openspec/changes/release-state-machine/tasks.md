# Tasks

The pull request's CI verifies every compile, format, lint, and test task.

On 2026-11-09 the operator limited 0.8.0 to the breaks that can be built soon.
The larger rewrites ship later in increments (operator, 2026-11-09).

The 0.8.0 slices are the changes [version-pr-proposal](../version-pr-proposal/proposal.md), [command-renames](../command-renames/proposal.md), [served-file-check](../archive/2026-11-10-served-file-check/proposal.md), and [release-commit-proofs](../archive/2026-11-10-release-commit-proofs/proposal.md), plus queue row [VER-036][queue].

Section 5 releases 0.8.0.
Sections 1 to 3 and 6 moved to six parts, which keep the task numbers.
The parts are [derived-release-state](../derived-release-state/tasks.md), [release-completion](../release-completion/tasks.md), [`version-pr-identity`](../version-pr-identity/tasks.md), [version-pr-content](../archive/2026-11-10-version-pr-content/tasks.md), [building-blocks](../building-blocks/tasks.md), and [built-in-ecosystems](../built-in-ecosystems/tasks.md).

0.4.0 to 0.7.4 shipped from `main` before this change.
The parts of it they carried are checked in each part with their evidence.
[audit.md](audit.md) lists the state of every breaking item.

State notes are from the code on `main` at [cb1be28][main], read on 2026-11-09 (UTC).

## 0. Decide

- [x] 0.1 The operator answers the open decisions in proposal.md.
  Proof: each answer is recorded on queue row [VER-011][queue] with `mise run q edit <row> -n …`.
  This change is updated where an answer differs from the recommendation.
  - 2026-10-12 (UTC), all eight approved as recommended, recorded in a note on [VER-011][queue]. Decision 8 (0.4.0 from `next`) is contradicted by what shipped, and the operator confirms the 0.8.0 scope after the audit.
- [x] 0.2 The held branch `feat/011-refresh-lockfile` is deleted unpushed once 0.1 is recorded; proof: `git branch --list feat/011-refresh-lockfile` prints nothing
  - 2026-11-09, `git branch --list feat/011-refresh-lockfile` and `git ls-remote origin refs/heads/feat/011-refresh-lockfile` print nothing.
- [ ] 0.3 Branch `next` is created from `main`; proof: `git ls-remote origin refs/heads/next` prints a commit
  - 2026-11-09, not started: no `next` exists. The 0.8.0 work is held on [a pull request stack][stack] instead, which waits on decision 8.

## 4. Config, served files, docs

- [ ] 4.1 The final schema: every removed key refused with its replacement, every struct refusing unknown keys, every name resolved at load.
  Proof: the release-config scenarios pass.
  - 2026-11-09, partly shipped: the config is read strictly through ctl-core `Input`, and 9 structs refuse unknown keys. Remaining: every key this change removes is still read, and none of its new keys exists.
- [ ] 4.2 `templates.rs` renders through `ctl_core::served`, with the `tags` map and the mount line in the context, after telling the session that owns [CTC-008][ctc-008]. `render` replaces `pin`. Operator docs and example workflows are served templates. Proof: served bytes are identical before and after for every template in this repository and qctl, and `check` fails on a hand-edited served file
  - 2026-11-09, not started.
- [ ] 4.3 AGENTS.md, README.md, `src/instructions.md`, the operator skill, and `docs/served-files.md` describe only 0.8.0. The instructions and the operator skill hold one rule.
  Whether a release publishes is read at its release commit, so a later `publish = false` does not cancel a merged version. Proof: `cargo test operator_docs` passes and no document names a removed key or command
  - 2026-11-09, not started.

## 5. Release 0.8.0

- [ ] 5.1 `release.yml` replaces `version-pr.yml` and `publish.yml` in this repository, and the asset jobs attest each archive.
  Proof: the 0.8.0 release passes CI, and `gh attestation verify` passes for each 0.8.0 asset
  - 2026-11-09, partly shipped: the reusable `release.yml` replaced `version-pr.yml` and `publish.yml` ([#166][pr166]). Remaining: attestation, which no workflow runs yet.
- [ ] 5.2 The 0.8.0 minor merges with one `minor` fragment per breaking change; proof: the 0.8.0 Version PR passes `check` on its first run, and the release goes from merge to a published Release with both assets; a rerun reports every step `already`
  - 2026-11-09, not started.
- [ ] 5.3 qctl, forkctl, ctl-core, unixd, and yamled move to 0.8.0.
  Delete `prepare`, and adopt the two workflows.
  qctl and forkctl move four pinned files to templates.
  Proof: each repository's next Version PR commits its lockfile and passes `check`.
  - 2026-11-09, not started.

## 7. Close

- [ ] 7.1 Archive these rows when 0.8.0 or the patch that removes their cause releases. The rows are [VER-013][queue], [VER-016][queue], [VER-020][queue], [VER-028][queue], [VER-029][queue], [VER-030][queue], and [VER-031][queue].
  More rows are [VER-033][queue], [VER-034][queue], [VER-035][queue], [VER-036][queue], [VER-038][queue], and [VER-010][queue].
  The evidence is that pull request. Proof: `mise run q check` passes
- [ ] 7.2 Archive this change and queue row [VER-011][queue] after every part is archived; proof: `openspec validate --specs` passes

[queue]: ../../../tasks.yaml
[ctc-008]: https://github.com/victor-software-house/ctl-core/blob/8a4095fbe4d30b5ea067682f13478be0ba21fbac/tasks.yaml
[main]: https://github.com/victor-software-house/verctl/commit/cb1be28
[pr166]: https://github.com/victor-software-house/verctl/pull/166
[stack]: https://github.com/victor-software-house/verctl/pull/256
