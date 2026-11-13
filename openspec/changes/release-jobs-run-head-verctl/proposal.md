# Release jobs run the verctl that HEAD pins

## Motivation

The reusable `release.yml` checks out each pending version commit in its publish, asset, and docs jobs.
Each job runs `mise run ver <verb>`, and the served `ver` task pins verctl through `#MISE tools`.
So each job runs the verctl that the old commit pins, which can be older than the workflow itself.

The plan job refuses a verctl older than 0.8.0, but it checks only HEAD.
A pending version commit that pins verctl 0.7.4 fails in publish with `unrecognized subcommand 'release'`, because 0.7.4 has no `release` verb.

The first fix ran the plan's verctl through `mise exec` with `MISE_LOCKED=false` and `MISE_LOCKFILE=false`, because the version commit's `mise.lock` lacks that version.
A consumer whose `mise.toml` sets `[settings] locked = true` and `[tool_config] locked = true` fails that step with `locked mode requires lockfile to be enabled`.
Every tool comes from a lock, and no lock is ever switched off.

Queue row: [VER-123][ver-123].

## Changes

1. The plan job outputs the verctl version it ran, which HEAD pins.
   HEAD's `mise.lock` lists that version.
2. The publish, asset, and docs jobs check out HEAD's mise files into `.verctl-head`, install that version with `mise -C .verctl-head install --locked`, and run the installed binary through `mise exec`.
3. A run that passes `verctl-artifact` keeps running that build, as today.
4. The workflow header states that every job runs the plan's verctl, installed from HEAD's lock.

## Capabilities

### New Capabilities

- `release-workflow-verctl`: which verctl each job of the reusable release workflow runs, and where its install comes from.

### Modified Capabilities

None.

## Impact

1. Workflow: `.github/workflows/release.yml` only.
2. Callers: none must change. A caller whose old version commit pins an older verctl releases it with HEAD's verctl, under the caller's own `locked` settings.
3. Release: a patch. The reusable workflow is referenced by commit, so a caller picks this up when it moves its ref.

[ver-123]: ../../../tasks.yaml
