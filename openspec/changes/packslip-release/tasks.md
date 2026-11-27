# Tasks

The pull request's CI verifies every compile, format, lint, and test task.

## 1. Configuration

- [ ] 1.1 `packslip` parses as a tracked path, and its bundle joins the declared assets of the Release that carries the tarballs.
  Verify: the `config` and `pending` packslip tests pass in CI, including the untracked and omitted cases.
- [ ] 1.2 An asset target's `tarball` template renders, the default name is unchanged, and two targets with one name fail the load.
  Verify: the `assets` name tests pass in CI, and the existing asset snapshots are unchanged.

## 2. Release verbs

- [ ] 2.1 `status --github-output` outputs `uploaded`, the declared tarballs already uploaded to the draft, empty with no draft.
  Verify: the `pending` uploaded tests pass in CI against a mocked API.
- [ ] 2.2 `verctl release` uploads `packslip.sigstore.json` and keeps the draft until it is uploaded.
  Verify: the `publish` bundle tests pass in CI.

## 3. Workflow

- [ ] 3.1 `plan` outputs the manifest path, `uploaded`, the asset tag and its version commit; `publish` runs the action pinned at v1.11.5 by commit with `download` set to `uploaded`, then `release`, and requests `attestations: write`.
  Verify: `mise run lint:workflows` and `mise run lint:workflow-policy` pass in CI.
- [ ] 3.2 `self-release.yml` grants `attestations: write`, and `.ctl/packslip.toml` declares `verctl`, its bash, zsh and fish completions, and its skill.
  Verify: the release run after merge publishes `packslip.sigstore.json`, and `packslip verify` accepts it for `github.com/victor-software-house/verctl`.

## 4. Consumers

- [ ] 4.1 mise installs and locks the release on every platform it ships.
  Verify: a workflow on `ubuntu-26.04`, `ubuntu-26.04-arm` and `xcode-27` runs `mise use packslip:github.com/victor-software-house/verctl@<version>`, then `mise lock --platform linux-x64,linux-arm64,macos-arm64`, and the lock lists all three.
- [ ] 4.2 The first TAB completes through mise in bash, zsh and fish.
  Verify: the same workflow activates mise in each shell, types `verctl`, a space and a TAB through a pseudo-terminal, and sees the verb list.
- [ ] 4.3 `mise skills ls` lists the `verctl` skill for the installed version.
  Verify: the same workflow prints it.

## 5. Docs

- [ ] 5.1 The operator instructions, the skill, the README and `examples/ver.yaml` describe `packslip` and `tarball`.
  Verify: `cargo test operator_docs` passes in CI.
