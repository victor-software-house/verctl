---
verctl: patch
---

The release workflow takes a `bootstrap` input, and `verctl release --bootstrap` creates each crate crates.io lacks with `VERCTL_REGISTRY_TOKEN` inside the release run, while trusted publishing publishes the rest.
