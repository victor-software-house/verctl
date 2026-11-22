---
verctl: patch
verctl:group: fixed
---

The version step skips a lockfile owner when the lockfile records no version for the bumped package, as the root workspace of a `bun.lock` does.
A private dependency that needs a token verctl withholds cannot fail the Version PR job through that install, and the lockfile stays out of the Version PR.
