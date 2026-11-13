---
verctl: patch
verctl:group: changed
---

`verctl release asset` takes `--prebuilt <path>` (`VERCTL_PREBUILT`), which packs a binary built earlier as the tarball's one entry and runs neither `prepare` nor `build`.
A target that packs `files` under `dir` refuses it, and it does not combine with `--vendored`.
In verctl's own release, the asset job reuses the release build when the version commit is the run's commit, and an older version commit still compiles from its own tree.

The default GitHub-hosted Ubuntu machines move to `ubuntu-26.04` and `ubuntu-26.04-arm`: the `runner` input of `release.yml`, the built-in `linux-x64` target, and the `ci` default.
A Linux tarball built on the defaults needs the newer glibc of Ubuntu 26.04.
