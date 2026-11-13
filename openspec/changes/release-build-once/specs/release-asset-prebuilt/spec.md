## ADDED Requirements

### Requirement: release asset packs a prebuilt binary

`verctl release asset <target> --prebuilt <path>` (`VERCTL_PREBUILT`) SHALL run neither `assets.prepare` nor `assets.build`.
It SHALL copy the file at `<path>` into a fresh directory as `{bin}` with mode `0755`, and write the tarball the target names, holding one entry named `{bin}`.
It SHALL fail when `<path>` is not a file.
It SHALL fail, naming the target, when the target packs `files` under `dir`.
It SHALL conflict with `--vendored`.
An empty `VERCTL_PREBUILT` SHALL count as unset.

#### Scenario: A build that would fail is skipped

- **WHEN** `.ctl/ver.yaml` declares `assets.build: ["false"]` and `linux-x64: {}`, and `verctl release asset linux-x64 --dry-run --prebuilt build/verctl` runs
- **THEN** it writes `verctl_1.2.3_linux_x64.tar.gz` holding one executable entry `verctl`, and does not run `false`

#### Scenario: A target that packs files

- **WHEN** `.ctl/ver.yaml` declares `assets.dir: out/{triple}` and `assets.files: [verctl, LICENSE]`, and `verctl release asset linux-x64 --prebuilt build/verctl` runs
- **THEN** it exits non-zero with an error naming `linux-x64` and `--prebuilt`, and writes no tarball

#### Scenario: A missing file

- **WHEN** `verctl release asset linux-x64 --prebuilt build/absent` runs and `build/absent` does not exist
- **THEN** it exits non-zero with `--prebuilt build/absent is not a file`

#### Scenario: A vendored set

- **WHEN** `verctl release asset linux-x64 --prebuilt build/verctl --vendored libexample` runs
- **THEN** it exits with a usage error for the conflict

#### Scenario: An empty variable

- **WHEN** `VERCTL_PREBUILT` is set to an empty string and `verctl release asset linux-x64` runs
- **THEN** the target builds as before

### Requirement: The reusable workflow reuses this run's build for its own commit

When `verctl-artifact` is set and an `assets` leg's version commit is the run's commit, `release.yml` SHALL pass the downloaded build to `release asset` as `VERCTL_PREBUILT`.
A leg for any other version commit, and every run without `verctl-artifact`, SHALL leave `VERCTL_PREBUILT` empty and compile as before.

#### Scenario: The version commit is the run's commit

- **WHEN** `self-release.yml` runs at the Version PR's merge commit and the plan lists that commit
- **THEN** the `assets` leg runs `verctl release asset` with `VERCTL_PREBUILT=<runner.temp>/verctl-build/verctl` and runs no `cargo build`

#### Scenario: An older pending commit

- **WHEN** the plan also lists an earlier version commit
- **THEN** that commit's `assets` leg leaves `VERCTL_PREBUILT` empty and compiles from its own tree

### Requirement: The default GitHub-hosted Ubuntu machines are 26.04

The default `runner` input of `release.yml` SHALL be `["ubuntu-26.04"]`.
The built-in `linux-x64` target SHALL run on `ubuntu-26.04`.
A `ci` section that declares no job SHALL plan one `verify` on `ubuntu-26.04`.
This repository's `runners.arm64` SHALL be `ubuntu-26.04-arm`.

#### Scenario: Default labels

- **WHEN** `.ctl/ver.yaml` declares `assets: {linux-x64: {}}` and no `ci`
- **THEN** `verctl status --format json` lists `linux-x64` on `ubuntu-26.04` and one `verify` check on `ubuntu-26.04`
