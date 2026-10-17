---
verctl: patch
---

`publish` asks the registry before it runs a publisher, and again after it. A stock cargo package bound for crates.io is read from the crates.io sparse index; any other package runs its recipe's new `query` command (stock bun: `bun pm view {name}@{version} version`). A listed version is reported `already` and its publisher does not run, so a rerun after a partial failure no longer depends on the registry's wording. A publisher that exits zero while the registry never lists the version now fails. Set `query` on a package or a `publishers` entry for another registry, or `query: []` to keep the old stderr match.
