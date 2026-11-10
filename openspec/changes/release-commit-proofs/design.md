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

## 2. Flags named for what they allow

The ctl naming rule asks for a name that says what a flag does. The two
flags are `--allow-workflow-change` and `--allow-off-default-branch`, each
with its `VERCTL_` variable. A run that uses one prints
`allowed: <check>: <reason>` on stderr, and the JSON report carries
`allowed: [{check, reason}]`.

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
