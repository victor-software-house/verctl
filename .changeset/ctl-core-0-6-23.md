---
verctl: patch
---

verctl reads `.ctl/ver.yaml` and fragment frontmatter with ctl-core 0.6.23, which moves to serde-saphyr 2.0. An error inside an aliased YAML value now keeps the parser's own wording and points at that value, followed by where the anchor is defined.
