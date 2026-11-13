# Install only the release tools at a version commit

## Motivation

The reusable `release.yml` checks out each pending version commit in its publish and docs jobs.
Each job then runs `mise install --locked` on that commit's whole mise config.

mise 2026.11.6 refuses a locked download whose lockfile entry has no checksum ([`locked_require_checksum`][checksum], on by default).
A lock written before that rule can hold URLs without checksums, so every tool it names becomes a reason to fail.

shrub shows the cost:

1. Its version commit `1ecc0b5` (shrub 0.7.0, 2026-10-26) names 18 tools, and its `mise.lock` has no checksum for taplo 0.10.0.
2. Every Release run since 2026-11-11 fails at that install, before any publish step: [run 43043113288][run-1], [run 43137938076][run-2] and [run 43143923972][run-3].
3. shrub-parser 0.7.0 reached crates.io, and the other 7 crates of 0.7.0 are still at 0.6.2.
4. The Version PR job is skipped while that release is pending, so shrub cannot release anything.

The publish needs only its publisher's tools: `rust` for the built-in cargo publisher, and `bun` for the built-in bun publisher.
verctl itself already installs from HEAD's lock ([VER-123][ver-123]).
The docs job only waits for docs.rs, so it needs no tool beyond verctl.

Queue row: [VER-129][ver-129].

## Changes

1. At a version commit, the publish job installs only the tools its packages' publishers run, from that commit's lock.
   It runs its commands with `MISE_EXEC_AUTO_INSTALL=false`, so mise installs nothing more.
2. The docs job installs no tool at the version commit, beyond the plan's verctl.
3. A new `release-tools` input names the tools to install in place of that list.
   `release-tools: all` installs the whole config, as today.
4. A custom publisher names no tool verctl can know, so a package with one installs the whole config unless `release-tools` is set.
5. The asset jobs keep the whole install, because each asset recipe runs its own tools.
   [VER-124][ver-124] covers an asset recipe that broke at its version commit.
6. The workflow header states which tools each job installs, and from which lock.

## Capabilities

### New Capabilities

- `release-workflow-tools`: which tools each job of the reusable release workflow installs at a version commit, and how a caller changes that.

### Modified Capabilities

None.

## Impact

1. Workflow: `.github/workflows/release.yml`, and the `status --github-output` keys the plan job writes.
2. Callers: none must change. A caller whose old version commit holds a lock without checksums releases it.
3. Release: a patch. The reusable workflow is referenced by commit, so a caller picks this up when it moves its ref.

[checksum]: https://mise.jdx.dev/configuration/settings.html#locked_require_checksum
[run-1]: https://github.com/victor-software-house/shrub/actions/runs/43043113288
[run-2]: https://github.com/victor-software-house/shrub/actions/runs/43137938076
[run-3]: https://github.com/victor-software-house/shrub/actions/runs/43143923972
[ver-123]: ../../../tasks.yaml
[ver-124]: ../../../tasks.yaml
[ver-129]: ../../../tasks.yaml
