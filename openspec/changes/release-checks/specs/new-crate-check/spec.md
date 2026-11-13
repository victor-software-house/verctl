## ADDED Requirements

### Requirement: A crate new in the pull request waits for its bootstrap

The `registry` check SHALL apply the existence and owner proofs to each crate that both the checked commit and the default branch hold.

A crate that only the pull request adds SHALL pass both when crates.io lacks the name, with a notice that it needs `verctl release bootstrap` once it lands.
When crates.io holds the name, the owner proof SHALL apply to it as to any other crate.
A crate that the pull request removes or renames SHALL not be proven.

#### Scenario: A new crate on a pull request

- **WHEN** a pull request adds workspace member `demo-extra` at `0.0.0`, the default branch holds no `demo-extra`, crates.io has none, and `verctl check registry` runs in CI
- **THEN** it exits zero, with a notice that `demo-extra` needs `verctl release bootstrap` once it lands

#### Scenario: A new crate whose name crates.io holds

- **WHEN** `.ctl/ver.yaml` sets `owners: [any-victor]`, a pull request adds `demo-extra`, the default branch holds no `demo-extra`, and crates.io lists `demo-extra` owned by `alice`
- **THEN** `verctl check registry` exits non-zero, names `demo-extra`, the missing login `any-victor`, and the owners it found

#### Scenario: A new crate with owners set

- **WHEN** `.ctl/ver.yaml` sets `owners: [any-victor]`, a pull request adds `demo-extra`, which neither the default branch nor crates.io holds, and `verctl check registry` runs
- **THEN** it exits zero, runs no owner proof for `demo-extra`, and gives the bootstrap notice

#### Scenario: A crate the default branch holds and the registry lacks

- **WHEN** the default branch holds `demo-extra`, crates.io has none, and `verctl check registry` runs on any pull request
- **THEN** it exits non-zero, names `demo-extra` as missing from crates.io, and names `verctl release bootstrap demo-extra` as the fix
