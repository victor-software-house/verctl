# Breaking-change audit for 0.8.0

Read from the code on `main` at [cb1be28][main] (0.7.4) on 2026-11-09 (UTC).
Docs were not taken as evidence.
The 0.8.0 minor carries every breaking change verctl plans; later work ships as additive patches.
[Stack #258][stack] already holds two breaks for it: the library `Driver` enum gains a `Text` variant, and every growable public enum is sealed.

A break is a change to `.ctl/ver.yaml` keys or meaning, to commands, flags, `VERCTL_*` variables, exit codes, or JSON report fields, to the inputs of the reusable [`release.yml`][release-yml], to the paths or inputs of `actions/*`, or to the public API of the `verctl` crate.

## Breaking items

| Breaking change | State on main | What it breaks | Waits on |
|:--|:--|:--|:--|
| BREAKING 8: commands become `status`, `check`, `version`, `propose`, `release`, `release asset`, `release bootstrap`, `render`, `ci`, `instructions` | Not started. The verbs are `instructions`, `status`, `check`, `prepare`, `publish`, `pin`, `assets`, `ci`, `docs`, `semver` ([`src/cli.rs:34-52`][cli]). `docs` and `semver` shipped later and are missing from the planned list | Commands: `prepare`, `publish`, `pin`, `assets` are renamed or removed | Decisions 2 and 8; the place of `docs` and `semver` in the new list is not decided |
| BREAKING 9: one key per concern in `.ctl/ver.yaml` | Partly shipped. Strict parsing shipped: 9 structs refuse unknown keys, read through ctl-core `Input` ([`src/config.rs`][config]). Every key to remove is still there: `drivers`, `publishers`, `prepare`, `pins`, `patterns` ([`src/config.rs:517-569`][config]). None of `branch`, `registries`, `ecosystems`, `steps`, `at`, `proposal` exists. `runners`, `vendored`, `ci`, and `assets` shipped since and are absent from the plan | Config: removed and renamed keys | Decisions 5 and 6 |
| BREAKING 10: two workflows replace three; proposal branch `verctl/release/<base>` | Partly shipped. The branch shipped in [#161][pr161]. One reusable `release.yml` replaced `version-pr.yml` and `publish.yml` in [#166][pr166]. The three `actions/*` remain, and no attestation step exists | Workflow: `release.yml` inputs that the later tasks remove (`title`, `commit-message`); action paths if VER-043 moves them | Decisions 2, 3, 4 |
| `version` replaces `prepare`; report field `next` becomes `after` (`version-pr-content` task 1.4) | Not started | Commands and JSON | Decision 8 |
| `proposal` keys replace `VERCTL_PR_TITLE` and `VERCTL_COMMIT_MESSAGE` (`version-pr-identity` task 3.1) | Not started. Both variables shipped as the interface in [#155][pr155] | Commands: two variables; workflow: `title` and `commit-message` inputs | Decision 3 |
| Version PR label removed (`version-pr-identity` task 3.2) | Partly shipped. `check` knows the Version PR by identity and reads no label, and CI dropped the `labeled` and `unlabeled` types ([#188][pr188]). `prepare.version_label` still exists and labels the PR ([`src/config.rs:1047`][config]) | Config: `prepare.version_label` | Decision 3 |
| `render` replaces `pin`; templates through `ctl_core::served` (task 4.2) | Not started | Commands: `pin`; config: `pins`, `patterns` | Decision 5 |
| Bun publishes through verctl's own registry client (`built-in-ecosystems` task 2.3) | Not started | Config: bun publisher keys | Decision 7 |
| VER-085 package discovery (stack #258) | Specified only | Config: with workspace files present, inference adds packages an existing `packages` list leaves out. Commands: `check` fails on a changed package with no fragment. Library: `PackageSpec` shape | VER-085 open decisions |
| VER-086 release groups and tag shapes (stack #258) | Specified only | Config: a repository with more than one package moves from `v{version}` to `{name}/v{version}` unless its existing tags keep their shape; reading `.changeset/config.json` with no `groups`; `linked` fails the load. Library: tag plan types | VER-086 open decisions |
| VER-087 dependent ranges and publish order (stack #258) | Specified only | Commands: `prepare` rewrites explicit internal ranges and bumps dependents a patch; `publish` follows the dependency graph and fails on a cycle (VER-033) | VER-087 open decisions |
| VER-088 release immutability (stack #258) | Specified only | No surface break. Behaviour: tags become annotated (GitHub creates them lightweight from the Release today); a Release with assets stays a draft. `check --repository` and `tags.immutable` are additive | VER-088 open decision on pin updates |
| VER-020 served-file check | Not started | Commands: `check` fails on a hand-edited served file and on a file both a template and a `pins` entry claim | none |
| VER-035 proved release commit | Partly shipped: the ancestor check ([`src/publish.rs:557`][publish]) | Commands: `publish` refuses workflow drift or two version commits on the first-parent line | none |
| VER-036 a check that cannot run says so | Not started: `expand` still drops an argv part with an unknown `{placeholder}` ([`src/publisher.rs:209-220`][publisher]) | Config and commands: publisher argv with an unknown placeholder fails instead of losing the argument | none |
| VER-043 actions move to their own repositories | Not started | Action paths `victor-software-house/verctl/actions/<name>` | Blocked by VER-011 |
| VER-033 publish order follows the graph | Partly shipped: cargo workspace members publish in dependency order ([`src/members.rs`][members]); other packages publish in config order ([`src/publish.rs:26`][publish]) | Commands: order changes, and a cycle fails | Folds into VER-087 |
| VER-022 schema layer moves to ctl-core | Partly shipped: validators and `Input` live in ctl-core | Library: `verctl::templates` items move unless re-exported | Whether `verctl::templates` is supported API |
| VER-028 owner of a shared template | Not started | Config: namespaced template paths or a required owner export would break; an optional owner would not | VER-028 is itself the decision |
| First publish with a token beside trusted publishing (new, `built-in-ecosystems` task 6.5) | Not started. `release.yml` refuses `registry-token` with `crates-io-trusted-publishing` ([`release.yml:314`][release-yml]). Task 6.3 only detects a first publish under OIDC and stops | Nothing: the plan gains a field, and a combination that fails today starts to work | Whether 6.3's stop gives way to publishing with the token |

## Patch-safe rows

VER-021, VER-030, VER-034, VER-037, VER-044, and VER-078 add or document without breaking.
VER-015, VER-031, VER-070, VER-071, VER-074, VER-075, VER-076, VER-080, VER-081, VER-082, and VER-084 look done on main and are still queued.
Archiving them is the lead's call; for VER-070, VER-076, and VER-084 only the code was read, not each acceptance line run.

## Library API

`src/lib.rs` exports every module, and its public enums are exhaustive on main.
VER-075 added `Command::Semver` and `Report::Semver` in a patch, which is formally a library break.
Stack #258 seals the growable enums in 0.8.0, so later additions stay additive; whether earlier patch additions were acceptable is a policy question for the operator.

## Decisions

The proposal's eight decisions were approved as recommended on 2026-10-12 (VER-011 note).
The audit leaves them as recorded and marks in [proposal.md](proposal.md) which ones shipped code settled or contradicts.
Decision 8 (ship 0.4.0 from an integration branch `next`) does not match what shipped.
0.4.0 shipped on 2026-10-12 without this change, and 0.4.1 to 0.7.4 shipped from `main`.
No `next` branch exists; the 0.8.0 minor is held on stack #258 instead.

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
