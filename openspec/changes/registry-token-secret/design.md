# Design

## Decisions

1. **The caller maps the token by name.**
   The publish job reads only `secrets.registry-token`.
   A token stored in an environment is mapped by its stored name.
   The job running in that environment receives the environment's value ([run 42780696593][run]).
   Keeping the computed read with `secrets: inherit` was considered and lost: inherit hands every caller secret to the called workflow, and jactionlint flags the read.
2. **The declared secret keeps its name.**
   Declaring a fixed `CARGO_REGISTRY_TOKEN` secret, so an environment secret matches it by name, was considered and lost.
   The run shows the mapped key need not match, and the name differs by registry.
3. **`registry-token-env` stays, as the variable name only.**
   Cargo reads `CARGO_REGISTRY_TOKEN` for crates.io and `CARGO_REGISTRIES_<NAME>_TOKEN` for another registry ([Cargo config][cargo]), and other publishers read their own variable.
   Removing the input and always exporting `CARGO_REGISTRY_TOKEN` was considered and lost: a caller publishing to another registry would have no way to name its variable.
4. **The refusal of a token beside trusted publishing stays.**
   It no longer needs the guard that skipped the computed read under trusted publishing, because nothing is read unless the caller maps it.

5. **verctl hands the token to the publisher.**
   The publish step sets the token in `VERCTL_REGISTRY_TOKEN` or `VERCTL_TRUSTED_PUBLISHING_TOKEN`, and the variable name in `VERCTL_REGISTRY_TOKEN_ENV`, then runs the bare verb.
   `verctl release` fails before any other work when both tokens are set.
   `--registry-token-env` must be a POSIX variable name ([POSIX Name][posix]), checked as a usage error.
   Every command verctl starts runs without either token variable, and only the publisher and its registry `query` get the token, under the named variable.
   No flag takes the token, so it never reaches an argv.
   Keeping the shell step that exported the token was considered and lost: it repeated logic the CLI can own and test, and the export reached every process the job started.
   The `query` gets the token because a query against a private registry needs the same credential, as it had when the step exported the token to the whole job.

Override and disable path: leave `environment` empty to run publish in no environment.
Map no `registry-token` to publish with trusted publishing or with the credential the publisher's own config finds.

[run]: https://github.com/victor-software-house/verctl/actions/runs/42780696593
[cargo]: https://doc.rust-lang.org/cargo/reference/config.html#registriesnametoken
[posix]: https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/V1_chap03.html#tag_03_216
