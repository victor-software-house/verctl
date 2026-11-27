## ADDED Requirements

### Requirement: A declared packslip manifest makes the bundle a declared asset

When `.ctl/ver.yaml` sets `packslip`, verctl SHALL require the path to name a tracked file.
verctl SHALL then declare `packslip.sigstore.json` as an asset of the Release that carries the tarballs, and SHALL count that Release incomplete until the bundle is uploaded.

#### Scenario: A tracked manifest

- **WHEN** `.ctl/ver.yaml` sets `packslip: .ctl/packslip.toml`, the file is tracked, and targets `darwin-arm64` and `linux-x64` are declared for `demo` `1.2.0`
- **THEN** `verctl status --github-output` lists `demo_1.2.0_macos_arm64.tar.gz`, `demo_1.2.0_linux_x64.tar.gz` and `packslip.sigstore.json` as declared

#### Scenario: An untracked manifest

- **WHEN** `.ctl/ver.yaml` sets `packslip: .ctl/packslip.toml` and git does not track that file
- **THEN** the load fails and names the `packslip` field and the path

#### Scenario: No manifest

- **WHEN** `.ctl/ver.yaml` does not set `packslip`
- **THEN** no Release declares `packslip.sigstore.json`

### Requirement: An asset target may name its tarball

An asset target MAY set `tarball`, a file name template.
verctl SHALL render `{bin}`, `{version}`, `{os}` and `{arch}` in it, and SHALL use `{bin}_{version}_{os}_{arch}.tar.gz`, with `macos` for `darwin`, when it is unset.
Two targets that render the same name SHALL fail the load.

#### Scenario: A musl target beside a glibc one

- **WHEN** target `linux-x64` is declared with no `tarball`, and target `linux-x64-musl` sets `tarball: "{bin}_{version}_linux_x64_musl.tar.gz"`, for `demo` `1.2.0`
- **THEN** the declared tarballs are `demo_1.2.0_linux_x64.tar.gz` and `demo_1.2.0_linux_x64_musl.tar.gz`

#### Scenario: Two targets with one name

- **WHEN** two targets render `demo_1.2.0_linux_x64.tar.gz`
- **THEN** the load fails and names both targets

### Requirement: The plan lists the tarballs already uploaded to the draft

`verctl status --github-output` SHALL output `uploaded`, the declared tarballs already uploaded to the draft Release for the asset tag, separated by spaces.
It SHALL output an empty `uploaded` when no draft exists yet.

#### Scenario: A rerun after one target was uploaded

- **WHEN** the draft for `v1.2.0` has `demo_1.2.0_linux_x64.tar.gz` uploaded and lacks `demo_1.2.0_macos_arm64.tar.gz`
- **THEN** `uploaded` is `demo_1.2.0_linux_x64.tar.gz`, and `assets` plans only `darwin-arm64`

#### Scenario: A first run

- **WHEN** no Release exists for `v1.2.0`
- **THEN** `uploaded` is empty

### Requirement: release uploads the bundle before it publishes the draft

When the Release declares `packslip.sigstore.json`, `verctl release` SHALL upload it from `VERCTL_ASSETS_DIR` with the tarballs, and SHALL publish the draft only once every declared asset, the bundle included, is uploaded.

#### Scenario: The bundle is missing

- **WHEN** `VERCTL_ASSETS_DIR` contains every tarball for `v1.2.0` and no `packslip.sigstore.json`
- **THEN** `verctl release` uploads the tarballs, leaves the draft, and warns `draft <url> lacks packslip.sigstore.json`

### Requirement: The reusable workflow signs the packslip

When the plan names a packslip manifest, the reusable workflow's `publish` job SHALL run the pinned `jdx/packslip` action, then `verctl release`.
The action SHALL cover the tarballs in `VERCTL_ASSETS_DIR` and those `uploaded` lists, with that manifest, the Release's tag and version commit, `upload: false` and `attest: true`.
The job SHALL request `attestations: write`.

#### Scenario: A release with a manifest

- **WHEN** a caller with `packslip: .ctl/packslip.toml` runs the reusable workflow for `v1.2.0`
- **THEN** the published Release contains every declared tarball and `packslip.sigstore.json`, and `packslip verify` accepts the bundle for `github.com/<owner>/<repo>`

#### Scenario: A release without a manifest

- **WHEN** the caller's `.ctl/ver.yaml` sets no `packslip`
- **THEN** the `publish` job skips the signing step
