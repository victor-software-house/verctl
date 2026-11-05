---
verctl: patch
verctl:group: added
---

The reusable release workflow takes an `environment` input for its `publish` job, so a registry token can live only in a GitHub environment that admits the default branch. In an environment, with no `registry-token` secret, `publish` reads the secret named by `registry-token-env`, such as `CARGO_REGISTRY_TOKEN`, which the caller passes with `secrets: inherit`. Without `environment` nothing changes.
