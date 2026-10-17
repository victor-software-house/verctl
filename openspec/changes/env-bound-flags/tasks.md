# Tasks

- [ ] 1. Take ctl-core 0.6.21 and add `env_prefix = "VERCTL"` to the root parser; proof: `cli` unit tests parse through `env_under_flags` and pass on a build runner
- [ ] 2. Route every integration test spawn through `tests/common::verctl()`; proof: `git grep CARGO_BIN_EXE_verctl -- tests` names only the helper
- [ ] 3. Collapse the action run blocks; proof: `actions_set_variables_and_run_the_bare_verb` passes
- [ ] 4. Prove the binding through the binary; proof: `verctl_format_selects_json_and_the_flag_wins` and `an_empty_verctl_config_means_the_default` pass, and `mise run verify` passes on a build runner
- [ ] 5. Instructions, operator skill, AGENTS.md, and the changeset state the binding; proof: `cargo test operator_docs` passes
