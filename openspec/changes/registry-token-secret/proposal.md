# Pass the registry token as a named secret

## Why

The reusable `release.yml` reads the registry token with
`secrets[inputs.registry-token-env]` when the publish job runs in an
environment and no `registry-token` secret is mapped. That read looks a secret
up by a computed name.

It works only when the caller passes `secrets: inherit`, which hands every
caller secret to the called workflow.
jactionlint 2.5.0, which the shared template runs from v0.0.87, flags that line
twice: once for overprovisioned secrets and once for obfuscation.

The read came from [release-environment][re], on the belief that an
environment secret reaches a called workflow only under its stored name. A
throwaway run on 2026-11-10 showed otherwise ([run 42780696593][run]):

1. The caller mapped `named-token: ${{ secrets.ONLY_IN_ENV }}`, and only an
   environment held `ONLY_IN_ENV`.
2. The called job running in that environment received the value under
   `named-token`.
3. The same called job outside the environment received an empty string.

GitHub documents the same rule ([reuse workflows][reuse]). A caller can
therefore map the token by name, and neither the computed read nor
`secrets: inherit` is needed.

None of the eight callers of `release.yml`, this repository's included, passes
`secrets: inherit`, `environment`, or `registry-token-env` (read 2026-11-10).

Queue row: [VER-104][ver-104]. The operator placed this break in the 0.8.0 minor
(operator, 2026-11-10).

## What Changes

1. **BREAKING:** the publish job reads the token only from the
   `registry-token` secret. A caller that keeps the token in an environment
   maps it by its stored name, `registry-token: ${{ secrets.CARGO_REGISTRY_TOKEN }}`,
   beside `environment: <name>`. A caller that relied on `secrets: inherit` to
   reach an environment token gets no token.
2. `registry-token-env` stays with one job: it names the variable the
   publisher reads the token from. It no longer names a secret.
3. The publish job still refuses a `registry-token` together with
   `crates-io-trusted-publishing`.
4. With `environment` set, the App secrets now reach the propose job, because
   the caller keeps its `secrets:` map.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `release-environment`: the caller maps the token by name, and no secret is
  read by a computed name.

## Impact

1. Workflow: `.github/workflows/release.yml`. This repository's caller,
   `self-release.yml`, maps no token and passes no `secrets: inherit`, so it
   does not change.
2. Docs: the instructions and their template, `README.md`, and
   `examples/workflows/release.yml`.
3. Callers: none of the seven outside this repository passes what this change
   removes, so none has to change its call for this item.
4. Archive order: after [release-environment][re], whose spec this changes.

[re]: ../release-environment/proposal.md
[ver-104]: ../../../tasks.yaml
[run]: https://github.com/victor-software-house/verctl/actions/runs/42780696593
[reuse]: https://docs.github.com/en/actions/how-tos/reuse-automations/reuse-workflows
