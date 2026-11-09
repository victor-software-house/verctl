---
verctl: minor
verctl:group: changed
---

The library's `Driver` enum gains a `Text` variant, which reads `go.mod`, `VERSION`, and marker-line files. Every public enum that can grow, other than the command list and `Bump`, is now `#[non_exhaustive]`, so a later variant is no longer an API break. The command line does not change.
