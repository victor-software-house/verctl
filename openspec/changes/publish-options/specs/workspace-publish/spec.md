## MODIFIED Requirements

### Requirement: A virtual workspace publishes member by member

A package on the stock cargo publisher SHALL be published member by member
when both of these hold:

- neither it nor `publishers.cargo` replaces the upload command or declares a
  `query`;
- its `path` is a manifest with a `[workspace]` table and no `[package]`
  table.

A `publish` mapping of options SHALL NOT count as a replaced command.

Its upload SHALL run
`cargo publish --locked --manifest-path <path> --package <member>` once for
each workspace member whose `publish` is not `false`, each under its own
timeout. With `publish: {verify: false}` on the package, each of those
commands SHALL also carry `--no-verify`. A member with `publish = false`
SHALL never be published.

#### Scenario: Two crates and a tool crate

- **WHEN** `.ctl/ver.yaml` declares `demo` at `Cargo.toml`, a virtual manifest whose members are `core`, `app`, and `xtask`, and `xtask` sets `publish = false`
- **THEN** `verctl release --dry-run` lists `core` and `app` and not `xtask`

#### Scenario: An overridden upload keeps one command

- **WHEN** the same package declares `publish: [cargo, publish, --locked, --workspace]`
- **THEN** `verctl release` runs that one command

#### Scenario: Options reach every member

- **WHEN** the same package declares `publish: {verify: false}` and crates.io lists neither member
- **THEN** `verctl release` runs `cargo publish` once for `core` and once for `app`, each with `--package <member>` and `--no-verify`, and checks each against the crates.io index
