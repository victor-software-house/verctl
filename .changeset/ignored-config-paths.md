---
verctl: patch
---

The reusable `release.yml` takes `mise-ignored-config-paths`: colon-separated mise config paths, relative to the repository root, that every job but `propose` ignores, so the release jobs skip tools only the repository's own tasks need.
