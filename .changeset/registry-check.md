---
verctl: patch
---

`verctl check registry` proves, with no token, that each crates.io crate the default branch holds exists and lists every login the new `owners` key names.
It also reports whether the crate's newest version came through trusted publishing from the repository.
A crate only the pull request adds passes with a notice that it needs `verctl release bootstrap` once it lands.

`verctl check trust` reads an owner's token from `VERCTL_REGISTRY_TOKEN`.
It proves that a trusted publisher entry names the repository and the workflow file the new `release_workflow` key names, `release.yml` when omitted.
`owners` sits at the top level of `.ctl/ver.yaml`, and a package's own `owners` replaces it for that package.
