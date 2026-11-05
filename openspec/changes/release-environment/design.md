# Design

## Decisions

1. **Only `publish` takes the environment.**
   It is the only job that reads a registry token.
   Applying it to every job was considered and lost: `plan`, `assets`, and `docs` read no secret, and each environment job records a deployment and waits on the environment's protection rules.
   `propose` reads the Version PR's App credentials, which another change replaces, so it stays out.
2. **The token is read under its stored name inside an environment.**
   An environment secret reaches a called workflow only under its own name, and no secret can be named `registry-token`.
   `registry-token-env` already names the token's variable, so it also names the secret.
   Declaring a fixed `CARGO_REGISTRY_TOKEN` secret was considered and lost: the name varies by registry.
3. **Outside an environment, and under trusted publishing, the stored-name read is off.**
   A caller that passes `secrets: inherit` and publishes by crates.io trusted publishing may also hold a `CARGO_REGISTRY_TOKEN` secret, in the repository or the environment, for a crate's first publish; reading it would trip the "not both" guard.

Override and disable path: omit `environment`, or pass the token as `registry-token`, which wins.
