# Design

## Context

See [proposal.md](proposal.md) for why, and [`release-workflow-verctl`](specs/release-workflow-verctl/spec.md) for what.

The served `ver` task holds `#MISE tools = { "github:victor-software-house/verctl" = "<version>" }` and runs `exec verctl "$@"`.
mise puts a task's own tools first on its PATH, so `mise run ver` always runs the version that the checked-out commit pins.
The plan job runs at HEAD, and the publish, asset, and docs jobs run at a version commit.

## Goals / Non-Goals

**Goals:**

1. Every job of one run uses the same verctl, the one the plan job checked against the workflow's minimum.
2. The caller's mise environment still applies to each job.

**Non-Goals:**

1. Changing the served `ver` task. Callers keep it for local use and for the plan and propose jobs.
2. Building the old commit with HEAD's toolchain. The old commit's other pins still apply.

## Decisions

### 1. The plan job outputs the checked verctl version

The "Require verctl 0.8.0 or later" step already reads `mise run --quiet ver --version`.
It writes the version to a step output, and the job exposes it as `verctl-version`.
A run with `verctl-artifact` leaves it empty, because that build is already the run's verctl.

### 2. Later jobs run `verctl` through `mise exec` at that version

```yaml
env:
  VERCTL_TOOL: ${{ needs.plan.outputs.verctl-version && format('github:victor-software-house/verctl@{0}', needs.plan.outputs.verctl-version) || '' }}
run: mise exec ${VERCTL_TOOL:+"$VERCTL_TOOL"} -- verctl release
```

`mise exec <tool>@<version>` puts that version first and still loads the caller's mise environment.
With no tool named, `mise exec -- verctl` runs the artifact build that an earlier step put first on PATH.

Alternatives considered:

1. **Upload the plan job's binary as an artifact.** It lost because the asset jobs run on other operating systems and architectures.
2. **Run `verctl` from PATH after `mise install`.** It lost because the caller's mise `[env]` would no longer apply.
3. **Check the minimum in every job.** It lost because a released commit cannot move its pin, so the check would only fail earlier.

## Risks / Trade-offs

1. **A caller with `locked = true` and no lockfile entry for that version.** `mise exec` installs a tool the lockfile may not list.
   Mitigation: the first caller run after this change is the proof, recorded in tasks.
2. **The verctl at HEAD meets an old commit's config.** That is the same pairing [release-asset-targets](../release-asset-targets/proposal.md) handles: an old release is read with HEAD's verctl.
