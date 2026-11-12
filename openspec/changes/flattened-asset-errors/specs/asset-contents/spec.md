## MODIFIED Requirements

### Requirement: A target's files are validated at load

The load SHALL fail, naming the target, in any of these cases:

- the target declares `files` and `assets.dir` is unset;
- the target declares an empty `files`;
- the target declares `files` and `assets.binary` is set;
- a path in the target's `files` leaves the repository.

`assets.dir` SHALL load without `assets.files` when every target declares its own `files`.

A problem in a target SHALL name the path the file holds, `assets.<target>.<field>`, and SHALL be placed on the line of that field.

#### Scenario: Target files without a directory

- **WHEN** `assets` declares `linux-x64: {files: [demo]}` and no `dir`
- **THEN** loading `.ctl/ver.yaml` fails with a message that names `linux-x64` and `assets.dir`

#### Scenario: A list on every target

- **WHEN** `assets` declares `dir: out`, no `files`, `linux-x64: {files: [demo]}`, and `darwin-arm64: {files: [demo]}`
- **THEN** `.ctl/ver.yaml` loads

#### Scenario: A target's problem on its own line

- **WHEN** `.ctl/ver.yaml` declares one package on lines 1 to 3, then `assets.linux-x64.files: [demo]` as block mappings on lines 4 to 6, with no `dir`
- **THEN** loading it fails with `.ctl/ver.yaml:6:12: assets.linux-x64.files: must come with assets.dir`
