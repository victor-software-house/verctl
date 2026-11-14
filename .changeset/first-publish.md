---
verctl: patch!
---

`verctl check registry` fails a never-published standalone package whose version is not `0.0.0`, naming the crate, its version, and `0.0.0`, where it passed before.
A workspace member is exempt, since it first publishes at the group's version.
The release proof runs `registry`, so `release` stops on it too.

`verctl release bootstrap [CRATE]...` publishes each crate crates.io lacks, at the version the default branch's tip holds, with the operator's `VERCTL_REGISTRY_TOKEN`.
It tags each published standalone package, then adds the trusted publisher entry of each published crate, and waits out crates.io's new-crate rate limit.

`verctl release trust [CRATE]...` lists each crate's trusted publisher entries and adds this repository's entry only when none matches, with an owner's `VERCTL_OWNER_TOKEN`.
`--dry-run` lists what either step would do and sends no write.
