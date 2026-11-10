# asset-contents Specification

## Purpose

Which files a release asset tarball packs: the one binary by default, or listed files from a build directory.
The list is shared by every target or set per target, and the config load checks it.

## Requirements

### Requirement: A package asset tarball packs the listed files

When `assets` declares `dir` and `files`, `verctl assets --build <id>` SHALL run `prepare` and `build`, then write `tar czf <asset> -C <dir> <files…>` at the repository root, with `{bin}`, `{version}`, `{id}`, `{os}`, `{arch}`, and `{triple}` replaced in `dir` and each file.
It SHALL fail naming the path when a listed file is missing after the build.

#### Scenario: Four files from the triple's directory

- **WHEN** `assets` declares `dir: out/{triple}`, `files: [demo, demod, "demod@.service", LICENSE]`, and target `linux-x64`, and the four files exist under `out/x86_64-unknown-linux-gnu`
- **THEN** `verctl assets --build linux-x64` writes `demo_1.2.3_linux_x64.tar.gz` whose entries are `demo`, `demod`, `demod@.service`, and `LICENSE`

#### Scenario: A listed file is missing

- **WHEN** `demod@.service` is absent after the build
- **THEN** `verctl assets --build linux-x64` fails with a message that names `demod@.service` as missing after the build

### Requirement: The listed files are validated at load

The load SHALL fail in any of these cases:

- `assets.dir` is set without a non-empty `assets.files`, and a target declares no `files` of its own;
- `assets.files` is set without `assets.dir`;
- `assets.binary` is set beside either;
- `dir` or a `files` entry leaves the repository.

#### Scenario: Binary beside files

- **WHEN** `assets` declares `binary: out/{bin}`, `dir: out`, and `files: [demo]`
- **THEN** loading `.ctl/ver.yaml` fails with a message that names the fix: list the binary in `files`

### Requirement: Without listed files a tarball holds the one binary

When `assets` declares neither `dir` nor `files`, `verctl assets --build <id>` SHALL pack only `bin` from the directory holding `binary`, as before.

#### Scenario: Stock recipe

- **WHEN** `assets` declares `binary: out/{bin}` and target `linux-x64`
- **THEN** the tarball's only entry is `demo`

### Requirement: A target's own files replace the shared list

When an `assets` target declares `files`, `verctl release asset <target>` SHALL pack exactly those paths from `assets.dir`, with the placeholders replaced, in place of `assets.files`.  
It SHALL fail naming that target's list when one of its paths is missing after the build.  
A target without `files` SHALL pack `assets.files`.

#### Scenario: One target packs fewer files

- **WHEN** `assets` declares `dir: out/{triple}`, `files: [bench, proof]`, `linux-x64: {}`, and `darwin-arm64: {files: [proof]}`
- **THEN** `verctl release asset darwin-arm64` writes a tarball whose only entry is `proof`
- **AND** `verctl release asset linux-x64` writes a tarball whose entries are `bench` and `proof`

#### Scenario: A path in a target's list is missing

- **WHEN** `darwin-arm64` declares `files: [proof]` and `proof` is absent after the build
- **THEN** `verctl release asset darwin-arm64` fails with a message that names `assets.darwin-arm64.files` and `proof`

### Requirement: A target's files are validated at load

The load SHALL fail, naming the target, in any of these cases:

- the target declares `files` and `assets.dir` is unset;
- the target declares an empty `files`;
- the target declares `files` and `assets.binary` is set;
- a path in the target's `files` leaves the repository.

`assets.dir` SHALL load without `assets.files` when every target declares its own `files`.

#### Scenario: Target files without a directory

- **WHEN** `assets` declares `linux-x64: {files: [demo]}` and no `dir`
- **THEN** loading `.ctl/ver.yaml` fails with a message that names `linux-x64` and `assets.dir`

#### Scenario: A list on every target

- **WHEN** `assets` declares `dir: out`, no `files`, `linux-x64: {files: [demo]}`, and `darwin-arm64: {files: [demo]}`
- **THEN** `.ctl/ver.yaml` loads
