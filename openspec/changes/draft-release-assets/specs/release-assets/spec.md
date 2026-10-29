## ADDED Requirements

### Requirement: A Release with declared assets is a draft until every asset is uploaded

When a tag carries a declared asset, a target tarball or an `assets.files`
entry, `verctl publish` SHALL create the tag at the version commit, then the
GitHub Release as a draft. A `verctl publish` that finds the draft SHALL
publish it when every declared asset is `uploaded`, and SHALL otherwise leave
it a draft, report the assets it lacks, and succeed.

#### Scenario: A prebuilt installer

- **WHEN** `.ctl/ver.yaml` declares `assets.files: [build/App.pkg]` and the Version PR for `app` `1.2.0` has merged
- **THEN** `verctl publish` creates tag `v1.2.0` and a draft Release, and reports `draft <url> lacks App.pkg`
- **AND** after `verctl assets --upload --file build/App.pkg`, `verctl publish` publishes the Release

#### Scenario: One asset job failed

- **WHEN** `verctl` declares targets `darwin-arm64` and `linux-x64` and only the `linux-x64` tarball was uploaded
- **THEN** `verctl publish` leaves the Release a draft
- **AND** `verctl publish --plan` lists the version commit and the `darwin-arm64` asset job

#### Scenario: No assets

- **WHEN** `.ctl/ver.yaml` declares no `assets`
- **THEN** `verctl publish` creates the Release published, as before

#### Scenario: Assets skipped

- **WHEN** `verctl publish` runs with `VERCTL_SKIP_ASSETS=true`
- **THEN** it creates the Release published

### Requirement: Release lookups see drafts

verctl SHALL find a Release for a tag whether it is a draft or published, and
SHALL count an asset only when its state is `uploaded`. A draft Release SHALL
count as an incomplete release.

#### Scenario: A draft with every asset

- **WHEN** the Release for `v1.2.0` is a draft that holds every declared asset
- **THEN** `verctl publish --plan` lists its version commit with no asset job

### Requirement: A repository uploads its own declared files

`verctl assets --upload --file <path>` SHALL upload the file to the Release
for the plan's tag, or `--tag`, and SHALL refuse a path that `assets.files`
does not declare. The load SHALL fail when two entries share a file name.

#### Scenario: An undeclared file

- **WHEN** `assets.files` declares `build/App.pkg` and the operator runs `verctl assets --upload --file build/Other.pkg`
- **THEN** the command fails and says to declare the file under `assets.files`
