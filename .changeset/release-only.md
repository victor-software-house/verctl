---
verctl: patch
verctl:group: added
---

`publisher: none` releases a package on GitHub only: `publish` creates its tag and Release, and `assets` its tarballs, with no registry upload. A private binary, such as a Cargo package with `publish = false`, no longer needs a stand-in publish command. A `registry` or `publish` on the same package fails the load.
