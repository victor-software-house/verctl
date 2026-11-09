---
verctl: minor
verctl:group: changed
---

The library's `Driver` enum gains a `Text` variant, which reads `go.mod`, `VERSION`, and marker-line files. Every public enum that can grow is now `#[non_exhaustive]`, so a later variant is no longer an API break. The exceptions are `Bump`, the command list, and the report states the binary prints: `semver_check::Decision`, `publish::Outcome`, and `docsrs::DocsStatus`. The command line does not change.
