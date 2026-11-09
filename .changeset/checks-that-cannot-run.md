---
verctl: minor
verctl:group: changed
---

A publisher or query argument that uses a `{placeholder}` with no value fails and names it, where it was dropped from the command line: a typo such as `{registr}`, or `{registry}` on a package with no registry. Braces that are not a placeholder name, such as a JSON argument, pass through.

`publish` refuses when it cannot prove HEAD is on the default branch, where it skipped the proof: with no git repository, no HEAD commit, or no `origin` remote, the error names which.
