## ADDED Requirements

### Requirement: A Release with declared assets is a draft until every asset is uploaded

When a tag's Release declares an asset target, `verctl release` SHALL create the tag at the version commit through the git refs API, then the GitHub Release with `draft: true`.
A `verctl release` that finds the draft SHALL publish it when every declared asset is `uploaded`, keeping the prerelease and latest settings of its stage.
Otherwise it SHALL leave the draft, report `draft <url> lacks <names>` as a warning, and succeed.
The `releases` of the report SHALL list URLs only.

#### Scenario: Two declared targets

- **WHEN** `.ctl/ver.yaml` declares targets `darwin-arm64` and `linux-x64` and the Version PR for `demo` `1.2.0` has merged
- **THEN** `verctl release` creates tag `v1.2.0` and a draft Release, and warns `draft <url> lacks demo_1.2.0_macos_arm64.tar.gz, demo_1.2.0_linux_x64.tar.gz`
- **AND** after `verctl release asset darwin-arm64` and `verctl release asset linux-x64`, `verctl release` publishes the Release

#### Scenario: No declared asset

- **WHEN** `.ctl/ver.yaml` declares no asset target
- **THEN** `verctl release` creates the Release published at once, and creates no tag through the git refs API

#### Scenario: Assets skipped

- **WHEN** `verctl release` runs with `VERCTL_SKIP_ASSETS=true`
- **THEN** it creates the Release published at once, and publishes an existing draft without its assets

#### Scenario: A draft made by hand

- **WHEN** a draft for `v1.2.0` exists with every declared asset and no tag
- **THEN** `verctl release` creates the tag at the version commit, and publishes the draft with the title and notes of this run
- **AND** a tag that names another commit fails the run

### Requirement: Release lookups see drafts

verctl SHALL find a Release for a tag whether it is a draft or published, and SHALL count an asset only when its state is `uploaded`.
A draft Release SHALL count as an incomplete release, even when it holds every declared asset.

#### Scenario: A draft with every asset

- **WHEN** the Release for `v1.2.0` is a draft that holds every declared asset
- **THEN** `verctl status --github-output` lists its version commit and no asset job

#### Scenario: A draft hidden from the tag endpoint

- **WHEN** `GET /releases/tags/v1.2.0` answers 404 and `GET /releases` lists a draft with that `tag_name`
- **THEN** the lookup finds the draft

### Requirement: A draft at an older version is planned again

An earlier version SHALL count as released only when its registry lists it or its tag exists, and its Release is not a draft.
A draft at an earlier version SHALL become a pending commit with its missing targets and its own asset names, whether or not the Release of a newer version is complete.

#### Scenario: A newer version landed

- **WHEN** the Release for `v1.1.0` is published with its tarball and the Release for `v1.0.0` is a draft with none
- **THEN** `verctl status --github-output` lists the version commit of `1.0.0`, one asset job for it, and `demo_1.0.0_linux_x64.tar.gz` as its expected asset

### Requirement: A leg declares the assets the plan expects

`status --github-output` SHALL write `declared`, a map from each pending commit to its expected asset file names.
`verctl release --declared-assets` (`VERCTL_DECLARED_ASSETS`, comma separated) SHALL declare exactly those names, and without it the run SHALL declare the targets the commit's own config names.

#### Scenario: A target removed at HEAD

- **WHEN** the plan expects `demo_1.0.0_linux_x64.tar.gz` at a commit whose config also names `darwin-arm64`
- **THEN** a leg with `VERCTL_DECLARED_ASSETS=demo_1.0.0_linux_x64.tar.gz` drafts and publishes on that one asset

### Requirement: A job without registry credentials starts no publisher

`verctl release --releases-only` (`VERCTL_RELEASES_ONLY`) SHALL create and publish tags and Releases only.
It SHALL refuse a package whose version the registry does not list, before any upload.

#### Scenario: An unlisted package

- **WHEN** `release --releases-only` finds a package the registry does not list
- **THEN** it fails with `--releases-only starts no publisher` and starts no publisher

### Requirement: A published Release that lacks an asset depends on immutability

A published Release that lacks a declared asset SHALL stay incomplete with only the missing targets planned, when GitHub reports it `immutable: false`.
When GitHub reports it `immutable: true`, the Release SHALL count complete with no asset job planned, and `status` and `release` SHALL warn once, naming the tag, the missing assets, and that a later version must ship them.

#### Scenario: A mutable Release

- **WHEN** the published Release for `v1.2.0` is mutable and lacks `demo_1.2.0_darwin_arm64.tar.gz`
- **THEN** `verctl status --github-output` plans one asset job, for `darwin-arm64`

#### Scenario: An immutable Release

- **WHEN** the published Release for `v1.2.0` is immutable and lacks `demo_1.2.0_darwin_arm64.tar.gz`
- **THEN** `verctl status --github-output` plans no asset job and `verctl status` prints a warning naming `v1.2.0` and that asset

### Requirement: A run at a finished version commit does nothing

`verctl release` at a version commit whose releases are all complete SHALL report `nothing to release` and exit zero, even while another version commit is pending.
At a commit that is not a version commit it SHALL still fail before any side effect.

#### Scenario: A finalize leg after its Release finished

- **WHEN** `finalize` runs at version commit `bbb`, whose Release is published, while the draft at version commit `aaa` still lacks an asset
- **THEN** `verctl release` at `bbb` reports `nothing to release` and exits zero

### Requirement: The reusable workflow finalizes after the assets

`release.yml` SHALL run a `finalize` job after `assets` that runs `verctl release` at each pending version commit in order.
Its `plan` job SHALL request `contents: write`, so it lists draft Releases.

#### Scenario: The plan job reads a draft

- **WHEN** a draft Release holds every declared asset and the `plan` job runs with `contents: write`
- **THEN** the plan lists the draft's version commit with no asset job, and `finalize` publishes it
