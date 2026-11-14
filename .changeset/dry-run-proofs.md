---
verctl: patch
"verctl:group": fixed
---

`verctl release --dry-run` and `release bootstrap --dry-run` take the path of a real release through every read and stop before the first write: no draft, upload or tag.
A refusal the release would raise is the dry run's error too, with the same message, such as a tag at another commit, a published Release at another commit, a changed workflow or an off-branch HEAD, and a failed release proof stops it there, listing the proof rows beside the packages still to release.
Its rows come from the real plan: each upload it would make, each version the registry already lists as `already`, and each tag as `would create`, `would create draft` or `would complete`.
It needs `GITHUB_TOKEN` for the tag and Release reads and fails without it naming the variable, and it still needs no registry or owner token.
A real `release --new` or `release bootstrap` no longer reports the registry and trust notices of a crate it created, since the run resolved them; a dry run keeps them as the plan.
