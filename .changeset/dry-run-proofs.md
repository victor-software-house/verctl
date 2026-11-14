---
verctl: patch
"verctl:group": fixed
---

`verctl release --dry-run` and `release bootstrap --dry-run` take the path of a real release through every read and stop before the first write: no draft, upload or tag.
A refusal the release would raise is the dry run's error too, with the same message.
Such refusals include a tag at another commit, a published Release at another commit, a changed workflow and an off-branch HEAD.
A failed release proof stops the dry run there, listing the proof rows beside the packages still to release.
Its rows come from the real plan: each upload it would make, each version the registry already lists as `already`, and each tag as `would create`, `would create draft` or `would complete`.
It needs `GITHUB_TOKEN` for the tag and Release reads and fails without it naming the variable, and it still needs no registry or owner token.
A real run no longer reports the `registry` notices of a crate it created, and `release bootstrap` no longer reports the `trust` notice of a crate whose entry it added.
A dry run keeps those notices as the plan, and a plain `release --new` keeps the `trust` notice as the prompt to run `release trust`.
