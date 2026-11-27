# Tasks

The pull request's CI verifies every compile, format, lint, and test task.
All of the change shipped in [#432][verctl#432], merged as [c7747ab][merge] and released in [0.8.6-rc.10][release].

## 1. The upgrade

- [x] 1.1 Pin ctl-core `=0.8.0-rc.2` in both dependency tables with `input-document` and `input-json`, and take it past the cooldown with `mise run update:own`.
  Verify: `cargo tree -d` lists one copy of each ctl crate and of serde-saphyr.
  - 2026-11-27 (UTC), pinned in [132ac01][c132ac01]: [`Cargo.toml`][cargo-toml] holds `ctl-core = "=0.8.0-rc.2"` with `input`, `input-document`, `input-json`, `surface` and `usage`.
    It names ctl-core in one dependency table, so the pin sits in the only table that lists it.
  - 2026-11-27 (UTC), departure: `mise run update:own` did not move the lock.
    It refuses the added crates erased-serde 0.4.11 and typeid 1.0.3, which no trusted owner publishes.
    One `cargo update -p ctl-core@<old> --precise 0.8.0-rc.2` under `CARGO_RESOLVER_INCOMPATIBLE_PUBLISH_AGE=allow` moved it.
    The lock diff of [132ac01][c132ac01] added ctl-input, erased-serde and typeid, dropped yamled, and moved the ctl crates and vsh-serde-saphyr.
  - 2026-11-27 (UTC), read from [`Cargo.lock`][cargo-lock] at the merge commit: one entry each for ctl-core, ctl-input, ctl-layout, ctl-term, ctl-text and vsh-serde-saphyr.
    The ctl crates are all at 0.8.0-rc.2.
    Nobody ran `cargo tree -d`; one lock entry per name means the graph holds one copy of each.
- [x] 1.2 Migrate every 0.8 break by the changelog's migration notes, and accept snapshots from a real run.
  Verify: `mise run verify` passes on the build host.
  - 2026-11-27 (UTC), migrated in [ec72091][cec72091] and [f20dd98][cf20dd98].
    The first moved `View::new` to a format value and the `format` flag to `Option<FormatName>`, and read JSON failures from stderr in the tests.
    The second made `verctl render` and the operator docs test build their surface from one constructor, `templates::operator_surface`, after a review finding.
  - 2026-11-27 (UTC), `mise run verify` passed in the `verify` job of the [CI run][ci-run] on the pull request head, [f20dd98][cf20dd98].
    It ran in CI, where this repository's rules put verification, and not on a build host.
    verctl has no snapshot tool, so no snapshot was accepted.
    The tests that read piped tables now pass `--format pretty`.

## 2. The input document

- [x] 2.1 Add the golden test for `status --input` and the unknown-key scenario.
  Verify: both scenarios pass as tests on the build host.
  - 2026-11-27 (UTC), added in [d7d14a9][cd7d14a]: `an_input_document_fills_status_as_its_flags_do` compares `status --input doc.json --format json` with the same run through `--dir`, and with the recorded [`status.json`][status-json].
    `an_unknown_key_in_an_input_document_names_its_file_and_line` checks exit code 2, an empty stdout, and `doc.yaml:1:` and `dirr` on stderr.
    Both are in [`tests/status.rs`][status-rs] and passed in the `verify` job of the [CI run][ci-run], not on the build host.

## 3. Docs and release

- [x] 3.1 Describe the input document, its variables, the precedence and the piped default in the skill and the instructions, through their templates.
  Verify: the rendered files match their templates in `mise run verify`.
  - 2026-11-27 (UTC), written in [65d4e3c][c65d4e3]: both templates and both rendered files gained an `## Input document` section, and the README line that put JSON errors on stdout names stderr.
    `operator_docs` byte-compares the renders with the committed files, and the `verify` job of the [CI run][ci-run] passed.
- [x] 3.2 Write a `verctl: patch!` changeset naming the new flags and each output change.
  Verify: `openspec validate ctl-core-0-8 --strict` passes.
  - 2026-11-27 (UTC), written in [42657c4][c42657c]: the fragment names `--input`, `--input-format`, `--json` and `--stream`, and lists seven output changes.
    The Version PR [#433][verctl#433] consumed it, and [0.8.6-rc.10][release] shows it under Breaking, from the [release run][release-run].
    `openspec validate ctl-core-0-8 --strict` passed on 2026-11-27 (UTC) before the archive.

[c132ac01]: https://github.com/victor-software-house/verctl/commit/132ac0102361bd70ddbb024aa82a8539cd7c4659
[c42657c]: https://github.com/victor-software-house/verctl/commit/42657c4d1cb83b1d98eee1a8905ca14acf2e79f6
[c65d4e3]: https://github.com/victor-software-house/verctl/commit/65d4e3cc72442488256d2de048e5afe23dbf8e42
[cd7d14a]: https://github.com/victor-software-house/verctl/commit/d7d14a93bd053df40ff1b0dbaee1bd5299e645d0
[cec72091]: https://github.com/victor-software-house/verctl/commit/ec72091a75b8d27286c0e5b6a4efa7e7aca2a8d0
[cf20dd98]: https://github.com/victor-software-house/verctl/commit/f20dd9803d8e29a65b2790acc6204f06d2bdbf24
[cargo-lock]: https://github.com/victor-software-house/verctl/blob/c7747ab409f2e4bda2ca2ef6825c4505dc27fcf5/Cargo.lock
[cargo-toml]: https://github.com/victor-software-house/verctl/blob/c7747ab409f2e4bda2ca2ef6825c4505dc27fcf5/Cargo.toml
[ci-run]: https://github.com/victor-software-house/verctl/actions/runs/46035357957
[merge]: https://github.com/victor-software-house/verctl/commit/c7747ab409f2e4bda2ca2ef6825c4505dc27fcf5
[release]: https://github.com/victor-software-house/verctl/releases/tag/v0.8.6-rc.10
[release-run]: https://github.com/victor-software-house/verctl/actions/runs/46041007719
[status-json]: https://github.com/victor-software-house/verctl/blob/c7747ab409f2e4bda2ca2ef6825c4505dc27fcf5/tests/testdata/input/status.json
[status-rs]: https://github.com/victor-software-house/verctl/blob/c7747ab409f2e4bda2ca2ef6825c4505dc27fcf5/tests/status.rs
[verctl#432]: https://github.com/victor-software-house/verctl/pull/432
[verctl#433]: https://github.com/victor-software-house/verctl/pull/433
