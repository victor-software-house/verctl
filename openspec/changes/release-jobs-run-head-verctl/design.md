# Design

## Context

See [proposal.md](proposal.md) for why, and [`release-workflow-verctl`](specs/release-workflow-verctl/spec.md) for what.

The served `ver` task holds `#MISE tools = { "github:victor-software-house/verctl" = "<version>" }` and runs `exec verctl "$@"`.
mise puts a task's own tools first on its PATH, so `mise run ver` always runs the version that the checked-out commit pins.
The plan job runs at HEAD, and the publish, asset, and docs jobs run at a version commit.
The version commit's `mise.lock` lists the verctl that commit pins, and HEAD's lists the verctl the plan job ran.

## Goals / Non-Goals

**Goals:**

1. Every job of one run uses the same verctl, the one the plan job checked against the workflow's minimum.
2. Every install comes from a lock, and no job switches `locked` or `lockfile` off.
3. The caller's mise environment still applies to each job.

**Non-Goals:**

1. Changing the served `ver` task. Callers keep it for local use and for the plan and propose jobs.
2. Building the old commit with HEAD's toolchain. The old commit's other pins still apply.

## Decisions

### 1. The plan job outputs the checked verctl version

The "Require verctl 0.8.4 or later" step already reads `mise run --quiet ver --version` at HEAD, under the caller's locks.
It writes the version to a step output, and the job exposes it as `verctl-version`.
A run with `verctl-artifact` leaves it empty, because that build is already the run's verctl.

### 2. Each job installs that version from HEAD's lock

Each of the publish, asset, and docs jobs adds two steps after the version commit's checkout, only when `verctl-artifact` is empty and `verctl-version` is not:

1. `actions/checkout` of `${{ github.sha }}` into `.verctl-head`, a sparse, non-cone checkout of `mise.toml`, `mise.*.toml`, `mise.lock`, `mise.*.lock`, and `.miserc.toml`, with `persist-credentials: false`.
2. A step that runs, under the workflow's `MISE_ENV`:

   ```sh
   mise -C .verctl-head install --locked "github:victor-software-house/verctl@<version>"
   echo "VERCTL_BIN=$(mise -C .verctl-head which verctl --tool="github:victor-software-house/verctl@<version>")" >> "$GITHUB_ENV"
   ```

The job then runs `mise exec -- "${VERCTL_BIN:-verctl}" <verb>`.
`mise exec` still loads the caller's mise environment.
With `verctl-artifact`, `VERCTL_BIN` stays unset and the build on PATH runs.

`mise install --locked` takes its download URL and checksum from HEAD's lock, and the version commit's own `mise.lock` is not touched.
mise 2026.11.6 has `--locked` as a global flag, `-C` to run in another directory, and `which <bin> --tool <tool@version>` to print the binary a version provides.
Locally, an install of a version the lock lacks fails with `No lockfile URL found ... (--locked mode)`, so the lock is consulted.

The second checkout sits inside the workspace, because `actions/checkout` rejects a `path` outside `GITHUB_WORKSPACE`.
It runs after the version commit's checkout, whose clean (`git clean -ffdx` of its own path) would otherwise remove it.
The step adds `/.verctl-head/` to `.git/info/exclude`, so a publisher's or build's dirty check does not count the untracked directory.

Alternatives considered:

1. **`mise exec <tool>@<version>` with `MISE_LOCKED=false MISE_LOCKFILE=false`.** It lost because it switches the caller's locks off, and a caller with `locked = true` and `[tool_config] locked = true` fails with `locked mode requires lockfile to be enabled`.
2. **Upload the plan job's binary as an artifact.** It lost because the asset jobs run on other operating systems and architectures.
3. **Run `verctl` from PATH after `mise install`.** It lost because the caller's mise `[env]` would no longer apply to a tool mise does not find in the config.
4. **Check the minimum in every job.** It lost because a released commit cannot move its pin, so the check would only fail earlier.

## Risks / Trade-offs

1. **HEAD's lock lacks a platform of the runner.** `mise install --locked` then fails with the missing platform named, and the caller adds the platform to `lockfile_platforms`, as for any other tool.
2. **The verctl at HEAD meets an old commit's config.** That is the same pairing [release-asset-targets](../release-asset-targets/proposal.md) handles: an old release is read with HEAD's verctl.
