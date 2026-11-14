---
verctl: patch!
---

`verctl release --new` (`VERCTL_NEW`) lets a release create each crate crates.io lacks with `VERCTL_REGISTRY_TOKEN`, while trusted publishing publishes every other crate, and the reusable release workflow takes a `publish-new` input that runs it.
`verctl status --new` plans the same.
`--bootstrap` and the `bootstrap` input are gone, replaced by `--new` and `publish-new`.

`verctl release bootstrap [CRATE]...` is now `release --new` followed by `release trust` for each crate that run created.
It takes every option `release` takes, and needs `VERCTL_REGISTRY_TOKEN` and `VERCTL_OWNER_TOKEN`.
It no longer has its own upload, tag or rate-limit wait: the release's proofs, draft Release and stop-and-rerun behaviour apply.
