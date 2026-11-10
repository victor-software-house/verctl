## ADDED Requirements

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
