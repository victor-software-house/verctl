# Tasks

The pull request's CI verifies every compile, format, lint, and test task.
Compile, format, lint, and test tasks that must run before a push run on the build host instead of the authoring workstation.

## 1. The fix

- [ ] 1.1 verctl pins ctl-core 0.7.3, and `Cargo.lock` resolves it.
  Verify: `cargo metadata --locked` in CI resolves ctl-core 0.7.3.
- [ ] 1.2 `Config::from_input` declares `assets.targets` as a flattened field.
  Verify: the `config` test `an_asset_target_problem_sits_on_its_own_line` gets `.ctl/ver.yaml:6:12: assets.linux-x64.files: must come with assets.dir`.
- [ ] 1.3 The existing asset target tests name `assets.linux-x64.files` instead of `assets.targets.linux-x64.files`.
  Verify: `cargo test` in CI passes.

## 2. Bundle

- [ ] 2.1 A `.changeset/` patch fragment.
  Verify: `verctl check` in CI passes on the pull request.
