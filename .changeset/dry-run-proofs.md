---
verctl: patch
"verctl:group": fixed
---

`verctl release --dry-run` and `release bootstrap --dry-run` take the path of a real release through every read and stop before the first write: no draft, upload or tag.

- A refusal the release would raise is the dry run's error too, with the same message: a tag at another commit, a published Release at another commit, a changed workflow, or an off-branch HEAD.
- A failed release proof stops the dry run there, listing the proof rows beside the packages still to release.
- Its rows come from the real plan: each upload it would make, each version the registry already lists as `already`, and each tag as `would create`, `would create draft` or `would complete`.
- It needs `GITHUB_TOKEN` for the tag and Release reads, and fails without it naming the variable. It needs no registry or owner token.

A real run drops the `registry` notices of each crate it created, and `release bootstrap` drops the `trust` notice of each crate whose entry it added.
A dry run keeps those notices as the plan, and a plain `release --new` keeps the `trust` notice as the prompt to run `release trust`.
