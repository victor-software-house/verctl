# Tasks

## 1. The retry

- [ ] 1.1 Move `cooldown_rejections` and `COOLDOWN_OFF` into one module that `semver` and `release` import.
  Verify: the existing semver cooldown tests pass unchanged.
- [ ] 1.2 Retry a workspace member's upload once with the cooldown off when every refused crate is a member of the workspace.
  Verify: both scenarios pass as tests.

## 2. Docs and release

- [ ] 2.1 Update `src/instructions.md`, the verctl skill and `AGENTS.md` where they describe the workspace publish or the cooldown, and write a `verctl: patch` changeset.
  Verify: `openspec validate publish-sibling-cooldown --strict` passes and the doc tasks pass in CI.
