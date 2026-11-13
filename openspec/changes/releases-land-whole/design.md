# Design

The research note `docs/research/2026-11-13-atomic-releases.md` holds the evidence for each decision.
The operator made each decision on 2026-11-13.

## 1. Prove every upload before the first

**Decision.**
`check --release` runs every check that does not change a registry: crate existence, the trusted publisher, ownership, `cargo publish --workspace --dry-run`, the replay of `version`, and the asset builds.
`release` runs it again before its first upload, so drift between the merge and the release stops the run with nothing published.

**Alternative.**
Rely on the registry's answer at upload time, as today.
It lost because the first refusal comes after other crates are live, as with shrub 0.7.0.

**Override.**
None for the release; a check that cannot pass is fixed at its cause, or the release is abandoned through the unfreeze label (decision 5).

## 2. HEAD is the release reference

**Decision.**
A run releases the versions HEAD's manifests hold, from HEAD.
It never releases an older version, before a newer one or in the same run.
`status` lists HEAD's incomplete releases only.

**Alternative.**
Release every incomplete version commit oldest first, each at its own commit, as today.
It lost because an old commit's environment breaks with time, as shrub's lock did, and because a retry then ships code the Version PR never covered.

**Override.**
A later, separate opt-in may release a skipped version; this change adds none.

## 3. Order: draft, registries, then the tag

**Decision.**
The run builds the assets and uploads them to a draft Release, publishes to the registries in dependency order, then publishes the draft, which creates the tag.
A draft can still be changed or deleted, so every reversible step comes first.

**Alternative.**
Tag before the uploads, as `release-tag-order` requires today.
It lost because a tag then exists for a release that may never complete.

**Override.**
None: the order holds for every package kind.
A package with no registry check still tags after its upload, as today.

## 4. Two checks, named for their reach

**Decision.**
`check --registry` is cheap and runs on every pull request as a CI step of its own, since it needs the network.
`check --release` runs where the Version PR is proven: the merge group in tier 1, the Version PR in tier 2, a dispatch in tier 3.

**Alternative.**
One `release --check` whose depth depends on the pull request.
It lost because the name hides what runs where.

**Disable.**
A repository leaves `check --registry` out of its CI; `check --release` still runs inside `release`.

## 5. Freeze the default branch until the release completes

**Decision.**
While the default branch holds a version whose release is incomplete, `check --registry` fails and names the pending release.
It first waits for a running release workflow at the default branch's tip, within its timeout, so a pull request checked during a normal release passes.
HEAD therefore never moves past an unreleased version, and a retry runs at the version commit.

**Alternative.**
Publish inside the merge queue.
It lost because GitHub can drop a group after its last check passed, and a registry upload cannot be undone.

**Override.**
A pull request labelled `verctl: unfreeze` passes the freeze, for a release that can never complete.

## 6. Three enforcement tiers

**Decision.**

1. With a merge queue, `check --release` is required on the merge-group commit, which is the commit that lands under the `MERGE` method.
2. With rulesets and no queue, `check --release` is required on the Version PR with "require branches to be up to date", and `check --registry` on every pull request.
3. With neither, the operator dispatches `check --release` before merging; the freeze is reported, not enforced.

`status` names the tier from the default branch's rules.
No git hook is part of the design: a hook runs only in the pushing clone and on no merge through GitHub.

**Alternative.**
Require a merge queue.
It lost because private repositories get one only on Enterprise Cloud.

## 7. First publish: `release --first` and `trust`

**Decision.**
`verctl release --first` runs locally with the operator's token.
It publishes the real code of each package the registry lacks, at the version the default branch holds, tags it, and runs `verctl trust`.
A standalone package is at `0.0.0` then; a member of a shared-version group is at the group's version.

`verctl trust` registers the repository's release workflow as each crate's trusted publisher, through `POST /api/v1/trusted_publishing/github_configs`, or checks that it is registered.
On a 429 both wait until `Retry-After`.

**Alternative.**
Publish a placeholder to claim the name.
It lost because crates.io and npm forbid content that only reserves a name.

**Override.**
The `registry-token` secret with the new `first-publish` input lets the workflow publish a missing crate itself; trusted publishing stays the default.

## 8. Rate limits

Trusted-publishing uploads count against no crates.io limit, so a release of existing crates meets none.
Only a first publish meets the new-crate limit of 5, then 1 every 10 minutes, and `release --first` waits it out.
