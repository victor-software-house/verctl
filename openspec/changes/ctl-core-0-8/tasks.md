# Tasks

## 1. The upgrade

- [ ] 1.1 Pin ctl-core `=0.8.0-rc.2` in both dependency tables with `input-document` and `input-json`, and take it past the cooldown with `mise run update:own`.
  Verify: `cargo tree -d` lists one copy of each ctl crate and of serde-saphyr.
- [ ] 1.2 Migrate every 0.8 break by the changelog's migration notes, and accept snapshots from a real run.
  Verify: `mise run verify` passes on the build host.

## 2. The input document

- [ ] 2.1 Add the golden test for `status --input` and the unknown-key scenario.
  Verify: both scenarios pass as tests on the build host.

## 3. Docs and release

- [ ] 3.1 Describe the input document, its variables, the precedence and the piped default in the skill and the instructions, through their templates.
  Verify: the rendered files match their templates in `mise run verify`.
- [ ] 3.2 Write a `verctl: patch!` changeset naming the new flags and each output change.
  Verify: `openspec validate ctl-core-0-8 --strict` passes.
