---
verctl: patch
verctl:group: changed
---

verctl reads and writes git through `gix` instead of `git2`, so it builds without OpenSSL or Perl. `prepare --pr` now pushes with the `git` binary, which must be on PATH: the token reaches it through a credential helper that reads the push's environment, never argv or a URL, and the push skips repository hooks. Inside a git hook, verctl ignores `GIT_INDEX_FILE`, `GIT_WORK_TREE`, and `GIT_DIR` and never writes the index.
