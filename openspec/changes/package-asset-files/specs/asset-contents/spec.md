## ADDED Requirements

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

The load SHALL fail when `assets.dir` is set without a non-empty `assets.files`, when `assets.files` is set without `assets.dir`, when `assets.binary` is set beside either, or when `dir` or a `files` entry leaves the repository.

#### Scenario: Binary beside files

- **WHEN** `assets` declares `binary: out/{bin}`, `dir: out`, and `files: [demo]`
- **THEN** loading `.ctl/ver.yaml` fails with a message that says to list the binary in `files`

### Requirement: Without listed files a tarball holds the one binary

When `assets` declares neither `dir` nor `files`, `verctl assets --build <id>` SHALL pack only `bin` from the directory holding `binary`, as before.

#### Scenario: Stock recipe

- **WHEN** `assets` declares `binary: out/{bin}` and target `linux-x64`
- **THEN** the tarball's only entry is `demo`
