# Design

## 1. Three proofs

1. **Ancestry.** The release commit is an ancestor of the default branch,
   as `prove_default_history` checks today. VER-036 turns each of its
   silent skips into a refusal that names the missing fact: no repository,
   no HEAD, or no default-branch ref. This change adds the flag that allows
   a commit off the default branch.
2. **Workflow drift.** The tree of `.github/workflows/` at the release commit
   equals the tree at the default branch's tip. Equal tree ids are enough; no
   file is read.
3. **One version commit.** For each package `publish` releases, the
   first-parent line from the package's newest release tag to the release
   commit holds exactly one commit that changes its version. A package with
   no tag counts from the root.

The third proof keeps the catch-up order `publish --plan` already uses: the
workflow releases pending version commits oldest first, so each older one is
tagged before a newer one runs. Two version commits for one package mean a
version was skipped, and publishing the newer one would leave a gap in the
registry that no later run fills.

For that order to reach the skipped version, `publish --plan` lists it.
Before this change the plan found one version commit per package, the one
that set HEAD's version, so a skipped version never appeared.

The plan now walks back from each incomplete package's version along the
first-parent line, one version at a time. It lists each version with no tag
at the commit that set it, and that version's assets upload to its own tag.
The walk stops at a tagged version, at `0.0.0`, at a revision with no
version, or at a commit whose version is not strictly above its parent's as
semver. A revert or a version set again is not a skipped release, and listing
it would plan the same tag at a second commit. `publish` refuses a package that still has an older entry in the
plan, so the proof reads what the plan found and asks GitHub nothing more.

## 2. Flags named for what they allow

The ctl naming rule asks for a name that says what a flag does. The two
flags are `--allow-workflow-change` and `--allow-off-default-branch`, each
with its `VERCTL_` variable. A run that uses one reports it: an
`allowed  <check>: <reason>` row in the human view, and
`allowed: [{check, reason}]` in the JSON report. The library does not
print, and ctl-core owns the streams, so the report is the record; in a
workflow run it lands in the job log.

A check that cannot run refuses even with a flag set. Each flag allows
one finding, a commit off the branch or a changed workflow tree; a missing
repository, HEAD, or default-branch ref is not that finding.

The one-version-commit proof has no flag. A skipped version cannot be fixed
by publishing the next one; the fix is to release the older commit first.

Alternative considered: one `--allow` flag that takes a check name. It lost
because a workflow input per check reads plainly in a caller, and each
variable maps to one flag as every other verctl option does.

## 3. Workflow inputs reach verctl through env

`release.yml` declares both inputs as `boolean`, default `false`. Each
publish step sets `VERCTL_ALLOW_WORKFLOW_CHANGE: ${{ inputs.allow-workflow-change }}`
in its `env:` and runs the bare verb. No input is ever written into a `run:`
string. Dispatch inputs reach a workflow as strings over the REST API, and
the job holds write permissions. verctl parses the variable as clap parses
any boolean flag variable, so `"false"` is false.

## 4. Order inside publish

The proofs run after the plan and before any publisher, tag, or Release
call, so a refused run changes nothing anywhere.
