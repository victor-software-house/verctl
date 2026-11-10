---
verctl: minor
verctl:group: added
---

`verctl release` reads the registry token from `VERCTL_REGISTRY_TOKEN` or `VERCTL_TRUSTED_PUBLISHING_TOKEN`, and fails when both are set.  
It gives the token to each publisher and its registry `query` under `--registry-token-env` (`VERCTL_REGISTRY_TOKEN_ENV`, default `CARGO_REGISTRY_TOKEN`).  
No other command it starts gets either variable, and no flag takes the token.

The reusable `release.yml` sets these in its publish step and runs `mise run ver release`, so its inputs and secrets stay the same.  
Only its `plan` job now checks that the pinned verctl is 0.8.0 or later.  
A pending version commit that pins an older verctl fails on the `release` verb, so move its pin with the workflow ref.
