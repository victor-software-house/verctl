# Design

The research note `docs/research/2026-11-13-atomic-releases.md` holds the evidence for each decision.
The operator made each decision on 2026-11-13.

The change covers crates.io.
npm and GitHub Packages, Go modules, and other registries are horizon rows of the [work queue][queue].

## 1. Named checks that add up

**Decision.**
`verctl check [CHECK]...` takes the names `fragments`, `served`, `versions`, `registry` and `trust`.
Names add up and never overlap; each check runs once.
Bare `check` runs `fragments` and `served`, which need only the checkout, so the pre-commit hook and `verify` keep today's result.
`--all` (`-a`) runs every check, and fails `trust` when no owner's token is set.
A new check later is a new name, so no existing name starts needing more.

**Alternative.**
Additive flags, `--versions`, `--registry` and `--trust`, each with a short form.
It lost because `--versions` today replaces the run instead of adding to it, and every new check would take a new long and short flag.
`cargo deny check [advisories bans licenses sources]` is the prior art for the names.

**Disable.**
A repository runs only the names it wants; bare `check` stays offline.

## 2. `--versions` goes, with no hidden flag

**Decision.**
`check --versions` is removed, and `check versions` replaces it.
No hidden flag or rename message stays behind.

**Alternative.**
Keep `--versions` hidden, failing with a message that names `check versions`, as the renamed verbs `publish` and `pin` do.
It lost to the operator's rule of no hidden compatibility flags (operator, 2026-11-13).

**Migration.**
The shared `check:versions` task of the actions repository, which every repository on the template runs in `verify`, switches to `check versions` in the release that ships this change.

## 3. What a check can prove without a token

**Decision.**
crates.io serves a crate's trusted publisher entries only to an owner's token.

1. From public endpoints, the `registry` check proves existence and the owners that `owners` lists.
2. It reports, as evidence, whether the latest version came through trusted publishing from the repository.
3. The `trust` check, with an owner's token, proves the entry.

A missing entry can then come only from its removal, because `release --new` registers it ([`first-publish`][first-publish]).

**Alternative.**
Require an owner's token on every pull request.
It lost because that scope can also create and delete trusted publisher entries, so every repository would store a sensitive token.

**Disable.**
Leave out `owners`, and the owner proof is skipped with a notice.

## 4. `owners` once, with a per-package override

**Decision.**
The top-level `owners` key lists the logins every package must have as owners.
A package's own `owners` replaces it for that package.

**Alternative.**
Only per package.
It lost because a multi-crate repository would repeat one list in every entry, against verctl's rule of naming a shared thing once.
cargo-release has an `owners` key too, but it only adds missing owners and never fails a mismatch.

**Disable.**
Omit the key at both levels.

## 5. `release --dry-run` is the release proof

**Decision.**
`release --dry-run` (`-n`) runs `fragments`, `served`, `versions` and `registry`, and `trust` when an owner's token is set.
It also runs `cargo publish --workspace --dry-run`, the replay of `version`, and every asset build, and uploads nothing.
`release` runs the same proof before its first upload, so drift between the merge and the release stops the run with nothing published.
The flag exists today and only prints the plan; it now proves the release.

**Alternative.**
A `check --release` that covers every check plus the builds.
It lost because it overlaps the named checks, and `cargo publish --dry-run` and `release-plz release --dry-run` already name the same proof.

**Override.**
None for the release; a proof that cannot pass is fixed at its cause, or the release is abandoned through the unfreeze label (decision 6).

## 6. Freeze the default branch until the release completes

**Decision.**
While the default branch holds a version whose release is incomplete, the `registry` check fails and names the pending release.
It first waits for a running release workflow at the default branch's tip, within its timeout, so a pull request checked during a normal release passes.
HEAD therefore never moves past an unreleased version.

**Alternative.**
Publish inside the merge queue.
It lost because GitHub can drop a group after its last check passed, and a registry upload cannot be undone.

**Override.**
A pull request labelled `verctl: unfreeze` passes the freeze, for a release that can never complete.
The label follows the `tool: state` form of release-please's `autorelease: pending`.

## 7. Three enforcement tiers

**Decision.**

1. With a merge queue, the `release --dry-run` job is required on the merge-group commit, which is the commit that lands under the `MERGE` method.
2. With rulesets and no queue, it is required on the Version PR with "require branches to be up to date", and `check registry` on every pull request.
3. With neither, the operator dispatches `release --dry-run` before merging; the freeze is reported, not enforced.

`status` names the tier from the default branch's rules.
No git hook is part of the design: a hook runs only in the pushing clone and on no merge through GitHub.

**Alternative.**
Require a merge queue.
It lost because private repositories get one only on Enterprise Cloud.

[queue]: ../../../tasks.yaml
[first-publish]: ../first-publish/proposal.md
