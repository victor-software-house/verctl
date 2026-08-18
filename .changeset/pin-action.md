---
verctl: patch
---

New `actions/pin`: a consumer that declares `[[pins]]` gets them rewritten to
the versions just released and committed to the default branch by naming one
action, instead of copying release-critical bash. The reference
`examples/workflows/publish.yml` now shows the job, which is why consumers had
been missing it.
