---
verctl: patch!
---

`verctl check registry` fails a never-published standalone package whose version is not `0.0.0`, naming the crate, its version, and `0.0.0`, where it passed before.
A workspace member is exempt, since it first publishes at the group's version.
The release proof runs `registry`, so `release` stops on it too.

`verctl release trust [CRATE]...` lists each crate's trusted publisher entries and adds this repository's entry only when none matches, with an owner's `VERCTL_OWNER_TOKEN`.
`--dry-run` lists what it would add and sends no write.
