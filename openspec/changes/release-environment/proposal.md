# Proposal

## Motivation

A caller cannot keep its registry token in a GitHub environment that admits only the default branch.
The reusable workflow's jobs run in no environment, so an environment secret never reaches them.

The token must be a repository secret that any branch's workflow can read.
A caller cannot set `environment` on the job that calls a reusable workflow; only the called workflow's jobs can ([GitHub, reuse workflows][reuse]).

Two facts shape the fix, both checked with throwaway workflows in this repository on 2026-11-05:

1. A job whose `environment` expression is empty runs in no environment and records no deployment.
2. With `secrets: inherit`, a called job in an environment reads that environment's secret through `secrets[<stored name>]`; the same job outside the environment reads nothing.

An environment secret is never named `registry-token`, because secret names hold only letters, digits, and underscores.

Queue row: [VER-082][ver-082].

## Changes

1. `release.yml` gains an `environment` input, default empty, applied to the `publish` job, the only job that reads a registry token.
2. In an environment, when the `registry-token` secret is empty, `publish` reads the secret named by `registry-token-env`, such as `CARGO_REGISTRY_TOKEN`.
3. Without `environment`, every job and every secret read is unchanged.

## Capabilities

### New Capabilities

- `release-environment`: running the registry publish in a caller-named environment.

### Modified Capabilities

None.

## Impact

1. Workflow: [`.github/workflows/release.yml`][release].
2. Docs: [`src/instructions.md`][instructions] and its template, [`README.md`][readme], [`examples/workflows/release.yml`][example].
3. Callers: none change. A caller opts in with `environment` and `secrets: inherit`.
4. The Version PR's App credentials are out of scope; a separate change replaces them.

[reuse]: https://docs.github.com/en/actions/how-tos/reuse-automations/reuse-workflows
[release]: ../../../.github/workflows/release.yml
[instructions]: ../../../src/instructions.md
[readme]: ../../../README.md
[example]: ../../../examples/workflows/release.yml
[ver-082]: ../../../tasks.yaml
