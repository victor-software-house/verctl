---
verctl: patch!
---

`verctl check [CHECK]...` runs the checks it names, `fragments`, `served` and `versions`, each once, and the report lists each check that ran.
Bare `check` runs `fragments` and `served`, and `--all` (`-a`) runs every check.
`check --versions` is removed with no hidden flag: run `check versions`.
The hidden verbs `prepare`, `pin`, `publish` and `assets`, and the hidden `release --plan`, are removed too, and each now fails as an unknown command or flag.
