---
verctl: patch!
---

`verctl release` creates the tag last: it creates a draft Release at HEAD with no tag, uploads the declared tarballs to it from `--assets-dir`, publishes to the registries, and then publishes the draft, which creates the tag.
A failed upload leaves no tag, and a rerun resumes, skipping every version a registry lists.
`release asset` builds its tarball and uploads nothing, and loses `--tag`.
The reusable workflow builds the tarballs before the release job and drops the `finalize` job.
A hand-written workflow passes the tarballs to `actions/publish` through its new `assets-dir` input, and `actions/asset` loses its `token` and `tag` inputs.
