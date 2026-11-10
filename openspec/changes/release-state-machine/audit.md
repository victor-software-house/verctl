# Breaking-change audit for 0.8.0

Read from the code on `main` at [cb1be28][main] (0.7.4) on 2026-11-09 (UTC).
Docs were not taken as evidence.
The 0.8.0 minor carries every breaking change verctl plans; later work ships as additive patches.
[Its pull request stack][stack] already holds two breaks.
The library `Driver` enum gains a `Text` variant, and every growable public enum is sealed.

A break is a change to any of these:

1. `.ctl/ver.yaml` keys or meaning.
2. Commands, flags, `VERCTL_*` variables, exit codes, or JSON report fields.
3. The inputs of the reusable [`release.yml`][release-yml].
4. The paths or inputs of `actions/*`.
5. The public API of the `verctl` crate.

## Breaking items

Each item shows its state on main, what it breaks, and what it waits on.

1. BREAKING 8: commands become `status`, `check`, `version`, `propose`, `release`, `release asset`, `release bootstrap`, `render`, `ci`, `instructions`.
   - State on main: Not started.
     The verbs are `instructions`, `status`, `check`, `prepare`, `publish`, `pin`, `assets`, `ci`, `docs`, `semver` ([`src/cli.rs:34-52`][cli]).
     `docs` and `semver` shipped later and are missing from the planned list.
   - What it breaks: Commands: `prepare`, `publish`, `pin`, `assets` are renamed or removed.
   - Waits on: Decisions 2 and 8.
     The place of `docs` and `semver` in the new list is not decided.
2. BREAKING 9: one key per concern in `.ctl/ver.yaml`.
   - State on main: Partly shipped.
     Strict parsing shipped: 9 structs refuse unknown keys, read through ctl-core `Input` ([`src/config.rs`][config]).
     Every key to remove is still there: `drivers`, `publishers`, `prepare`, `pins`, `patterns` ([`src/config.rs:517-569`][config]).
     None of `branch`, `registries`, `ecosystems`, `steps`, `at`, `proposal` exists.
     `runners`, `vendored`, `ci`, and `assets` shipped since and are absent from the plan.
   - What it breaks: Config: removed and renamed keys.
   - Waits on: Decisions 5 and 6.
3. BREAKING 10: two workflows replace three; proposal branch `verctl/release/<base>`.
   - State on main: Partly shipped.
     The branch shipped in [#161][pr161].
     One reusable `release.yml` replaced `version-pr.yml` and `publish.yml` in [#166][pr166].
     The three `actions/*` remain, and no attestation step exists.
   - What it breaks: Workflow: `release.yml` inputs that the later tasks remove (`title`, `commit-message`).
     Action paths, if queue row [VER-043][queue] moves them.
   - Waits on: Decisions 2, 3, 4.
4. `version` replaces `prepare`; report field `next` becomes `after` (`version-pr-content` task 1.4).
   - State on main: Not started.
   - What it breaks: Commands and JSON.
   - Waits on: Decision 8.
5. `proposal` keys replace `VERCTL_PR_TITLE` and `VERCTL_COMMIT_MESSAGE` (`version-pr-identity` task 3.1).
   - State on main: Not started.
     Both variables shipped as the interface in [#155][pr155].
   - What it breaks: Commands: two variables; workflow: `title` and `commit-message` inputs.
   - Waits on: Decision 3.
6. Version PR label removed (`version-pr-identity` task 3.2).
   - State on main: Partly shipped.
     `check` finds the Version PR by identity and reads no label, and CI dropped the `labeled` and `unlabeled` types ([#188][pr188]).
     `prepare.version_label` still exists and labels the PR ([`src/config.rs:1047`][config]).
   - What it breaks: Config: `prepare.version_label`.
   - Waits on: Decision 3.
7. `render` replaces `pin`; templates through `ctl_core::served` (task 4.2).
   - State on main: Not started.
   - What it breaks: Commands: `pin`; config: `pins`, `patterns`.
   - Waits on: Decision 5.
8. Bun publishes through verctl's own registry client (`built-in-ecosystems` task 2.3).
   - State on main: Not started.
   - What it breaks: Config: bun publisher keys.
   - Waits on: Decision 7.
9. Queue row [VER-085][queue], package discovery ([the 0.8.0 pull request stack][stack]).
   - State on main: Specified only.
   - What it breaks: Config: with workspace files present, inference adds packages an existing `packages` list leaves out.
     Commands: `check` fails on a changed package with no fragment.
     Library: `PackageSpec` shape.
   - Waits on: the open decisions of [VER-085][queue].
10. Queue row [VER-086][queue], release groups and tag shapes ([the 0.8.0 pull request stack][stack]).
    - State on main: Specified only.
    - What it breaks:
      1. Config: a repository with more than one package moves from `v{version}` to `{name}/v{version}` unless its existing tags keep their shape.
      2. Config: reading `.changeset/config.json` with no `groups`; `linked` fails the load.
      3. Library: tag plan types.
    - Waits on: the open decisions of [VER-086][queue].
11. Queue row [VER-087][queue], dependent ranges and publish order ([the 0.8.0 pull request stack][stack]).
    - State on main: Specified only.
    - What it breaks: Commands: `prepare` rewrites explicit internal ranges and bumps dependents a patch.
      `publish` follows the dependency graph and fails on a cycle (queue row [VER-033][queue]).
    - Waits on: the open decisions of [VER-087][queue].
12. Queue row [VER-088][queue], release immutability ([the 0.8.0 pull request stack][stack]).
    - State on main: Specified only.
    - What it breaks: No surface break.
      Behaviour: tags become annotated (GitHub creates them lightweight from the Release today); a Release with assets stays a draft.
      `check --repository` and `tags.immutable` are additive.
    - Waits on: the open decision of [VER-088][queue] on pin updates.
13. Queue row [VER-020][queue], served-file check.
    - State on main: Not started.
    - What it breaks: Commands: `check` fails on a hand-edited served file and on a file both a template and a `pins` entry claim.
    - Waits on: none.
14. Queue row [VER-035][queue], proved release commit.
    - State on main: Partly shipped: the ancestor check ([`src/publish.rs:557`][publish]).
    - What it breaks: Commands: `publish` refuses workflow drift or two version commits on the first-parent line.
    - Waits on: none.
15. Queue row [VER-036][queue], a check that cannot run fails instead of passing.
    - State on main: Not started: `expand` still drops an argv part with an unknown `{placeholder}` ([`src/publisher.rs:209-220`][publisher]).
    - What it breaks: Config and commands: publisher argv with an unknown placeholder fails instead of losing the argument.
    - Waits on: none.
16. Queue row [VER-043][queue], actions move to their own repositories.
    - State on main: Not started.
    - What it breaks: Action paths `victor-software-house/verctl/actions/<name>`.
    - Waits on: Blocked by [VER-011][queue].
17. Queue row [VER-033][queue], publish order follows the graph.
    - State on main: Partly shipped: cargo workspace members publish in dependency order ([`src/members.rs`][members]); other packages publish in config order ([`src/publish.rs:26`][publish]).
    - What it breaks: Commands: order changes, and a cycle fails.
    - Waits on: Folds into [VER-087][queue].
18. Queue row [VER-022][queue], schema layer moves to ctl-core.
    - State on main: Partly shipped: validators and `Input` live in ctl-core.
    - What it breaks: Library: `verctl::templates` items move unless re-exported.
    - Waits on: Whether `verctl::templates` is supported API.
19. Queue row [VER-028][queue], owner of a shared template.
    - State on main: Not started.
    - What it breaks: Config: namespaced template paths or a required owner export would break; an optional owner would not.
    - Waits on: [VER-028][queue] is itself the decision.
20. First publish with a token beside trusted publishing (new, `built-in-ecosystems` task 6.5).
    - State on main: Not started.
      `release.yml` refuses `registry-token` with `crates-io-trusted-publishing` ([`release.yml:314`][release-yml]).
      Task 6.3 only detects a first publish under OIDC and stops.
    - What it breaks: Nothing: the plan gains a field, and a combination that fails today starts to work.
    - Waits on: Whether 6.3's stop gives way to publishing with the token.

## Patch-safe rows

Queue rows [VER-021][queue], [VER-030][queue], [VER-034][queue], [VER-037][queue], [VER-044][queue], and [VER-078][queue] add or document without breaking.

Queue rows [VER-015][queue], [VER-031][queue], [VER-070][queue], [VER-071][queue], [VER-074][queue], and [VER-075][queue] look done on main and are still queued.
Queue rows [VER-076][queue], [VER-080][queue], [VER-081][queue], [VER-082][queue], and [VER-084][queue] look done on main and are still queued.

Archiving them is the lead's call.
For [VER-070][queue], [VER-076][queue], and [VER-084][queue] only the code was read, and the acceptance lines were not all run.

## Library API

`src/lib.rs` exports every module, and its public enums are exhaustive on main.
Queue row [VER-075][queue] added `Command::Semver` and `Report::Semver` in a patch, which is formally a library break.
[The 0.8.0 pull request stack][stack] seals the growable enums in 0.8.0, so later additions stay additive.
Whether earlier patch additions were acceptable is a policy question for the operator.

## Decisions

The proposal's eight decisions were approved as recommended on 2026-10-12 (note on queue row [VER-011][queue]).
The audit leaves them as recorded and marks in [proposal.md](proposal.md) which ones shipped code settled or contradicts.

Decision 8 (ship 0.4.0 from an integration branch `next`) does not match what shipped.
0.4.0 shipped on 2026-10-12 without this change, and 0.4.1 to 0.7.4 shipped from `main`.
No `next` branch exists; the 0.8.0 minor is held on [a pull request stack][stack] instead.

[queue]: ../../../tasks.yaml
[main]: https://github.com/victor-software-house/verctl/commit/cb1be28
[stack]: https://github.com/victor-software-house/verctl/pull/256
[release-yml]: https://github.com/victor-software-house/verctl/blob/cb1be28/.github/workflows/release.yml
[cli]: https://github.com/victor-software-house/verctl/blob/cb1be28/src/cli.rs#L34-L52
[config]: https://github.com/victor-software-house/verctl/blob/cb1be28/src/config.rs
[publish]: https://github.com/victor-software-house/verctl/blob/cb1be28/src/publish.rs
[publisher]: https://github.com/victor-software-house/verctl/blob/cb1be28/src/publisher.rs#L209-L220
[members]: https://github.com/victor-software-house/verctl/blob/cb1be28/src/members.rs
[pr155]: https://github.com/victor-software-house/verctl/pull/155
[pr161]: https://github.com/victor-software-house/verctl/pull/161
[pr166]: https://github.com/victor-software-house/verctl/pull/166
[pr188]: https://github.com/victor-software-house/verctl/pull/188
