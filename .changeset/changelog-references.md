---
verctl: patch
---

Each changelog entry now links the pull request that merged its fragment, read from the trailing `(#N)` of the squash commit that added it, or that commit when the subject names none. Only outside contributors who are people get a byline; maintainers and bots never do, and without a token no one does.
