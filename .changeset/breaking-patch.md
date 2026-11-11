---
verctl: patch
verctl:group: added
---

A fragment may bump a package by `patch!`: a patch that breaks the package's API on purpose.
While the fragment is pending, `verctl semver` skips the package, and the changelog lists the entry under `### Breaking`.
The Version PR consumes the fragment, so the next release is checked again.
