# Tasks

- [x] 1.1 `git2` is gone and `deny.toml` bans `openssl` and `native-tls`; proof: `cargo deny check bans` in `mise run verify` on the build host
- [x] 1.2 The git module's tests pass on gix with `git` CLI fixtures; proof: `cargo test --all-features` on the build host
- [x] 1.3 Hook variables are ignored and the index is untouched; proof: `prepare_ignores_hook_variables` on the build host
- [x] 1.4 The push token reaches the server and not the error; proof: `push_keeps_the_token_out_of_the_error` on the build host
- [x] 1.5 Operator docs say `--pr` needs `git`; proof: `cargo test operator_docs` on the build host

2026-10-13 UTC: on a Namespace instance (no Perl, no OpenSSL headers installed), `cargo clippy --all-targets --all-features -D warnings`, `cargo test --all-features` (every suite, including `prepare_ignores_hook_variables`, `push_keeps_the_token_out_of_the_error`, and `operator_docs`), and `cargo deny check bans licenses sources` passed. The pre-push `verify` on the pushed commit is the remaining proof.
