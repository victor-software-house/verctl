## ADDED Requirements

### Requirement: A vendored set declares its own version and names

`.ctl/ver.yaml` SHALL accept a `vendored` mapping whose keys name asset sets.
Each set SHALL declare `version`, `dir`, `files`, and `targets`, and MAY declare `tag`, `archive`, `build`, and `notes`.
`tag` SHALL default to `{name}-{version}` and `archive` to `{name}-{version}-{id}.tar.gz`.

#### Scenario: Defaults name the tag and the archives

- **WHEN** `.ctl/ver.yaml` declares `vendored.libexample` with `version: 1.4.0` and targets `linux-x64` and `macos-arm64`
- **THEN** `verctl assets --vendored libexample --format json` reports tag `libexample-1.4.0` and assets `libexample-1.4.0-linux-x64.tar.gz` and `libexample-1.4.0-macos-arm64.tar.gz`

### Requirement: A vendored set is validated at load

The load SHALL fail when an `archive` template lacks `{id}` or does not end in `.tar.gz`, when `files` or `targets` is empty, or when `dir`, a `files` entry, or `notes` leaves the repository. The load SHALL also fail when a target's `runner` is not declared in `runners`, or when a set name, a target key, or `version` is empty, `.`, `..`, or holds `/` or `\`.

#### Scenario: Archive without the target

- **WHEN** a set declares `archive: libexample.tar.gz`
- **THEN** loading `.ctl/ver.yaml` fails with a message that says the archive must contain `{id}`

#### Scenario: Target key that leaves the repository

- **WHEN** a set declares a target `../x`
- **THEN** loading `.ctl/ver.yaml` fails with a message that says the key must be one plain name

### Requirement: A config needs a package or a vendored set

The load SHALL fail when `packages` and `vendored` are both empty, and SHALL succeed when only one of them is declared.

#### Scenario: Only vendored

- **WHEN** `.ctl/ver.yaml` declares `runners` and `vendored` and no `packages`
- **THEN** `verctl assets --vendored libexample` plans the set

### Requirement: Building a vendored target packs the listed files

`verctl assets --vendored <name> --build <id>` SHALL run the set's `build` argv with `{name}`, `{version}`, and `{id}` replaced, then write `tar czf <archive> -C <dir> <files…>` at the repository root.
It SHALL fail naming the path when a listed file is missing after the build.

#### Scenario: Library, headers, and license

- **WHEN** a set declares `dir: out/{id}` and `files: [lib/libexample.a, include, LICENSE]`, and those paths exist under `out/linux-x64`
- **THEN** `verctl assets --vendored libexample --build linux-x64` writes `libexample-1.4.0-linux-x64.tar.gz` whose entries start with `lib/libexample.a`, `include/`, and `LICENSE`

### Requirement: Uploading creates the Release when it is missing

`--upload` with `--vendored` SHALL create the set's Release at `HEAD` when the tag has none, named after the tag, with the `notes` file as its body, and with `make_latest` set to `false`.
It SHALL use an existing Release unchanged, including one another job created after verctl found none.

#### Scenario: First target

- **WHEN** no Release exists for `libexample-1.4.0` and a job runs `verctl assets --vendored libexample --build linux-x64 --upload`
- **THEN** verctl creates the Release with `make_latest: false` and attaches the archive

#### Scenario: Two jobs race to create

- **WHEN** GitHub answers the creation with 422 because the other target's job created the Release first
- **THEN** verctl reads that Release and uploads to it

### Requirement: Each archive ships a checksum file

After uploading an archive, verctl SHALL upload `<archive>.sha256` holding `<hex>  <archive>` and a newline, where `<hex>` is the SHA-256 GitHub reports as the archive's `digest`.

#### Scenario: Checksum from the stored archive

- **WHEN** the Release lists the archive with digest `sha256:abc123`
- **THEN** verctl uploads `libexample-1.4.0-linux-x64.tar.gz.sha256` with the content `abc123  libexample-1.4.0-linux-x64.tar.gz`

### Requirement: Existing assets are not uploaded again

When the Release already holds a complete asset of the archive's name or the checksum's name, the upload of that asset SHALL report `already` and SHALL NOT fail.

#### Scenario: Rerun after success

- **WHEN** a job reruns after both assets were uploaded
- **THEN** verctl reports both as `already` and exits zero

### Requirement: Package assets are unchanged

`verctl assets` without `--vendored` SHALL plan, name, pack, report, and upload exactly as before this change, and SHALL refuse `--vendored` together with `--tag`.

#### Scenario: Stock binary archive

- **WHEN** a config declares `assets.linux-x64` for package `demo` at `1.2.3`
- **THEN** `verctl assets --build linux-x64` writes `demo_1.2.3_linux_x64.tar.gz` holding only `demo`
