---
verctl: patch
---

A Release with declared assets is created as a draft after its tag, and a later `verctl publish` publishes it once every declared asset is uploaded, so releases work with GitHub immutable Releases. `assets.files` declares files a repository builds itself, and `verctl assets --upload --file <path>` uploads one. The reusable release workflow gains a `finalize` job after `assets`, and its `plan` job now takes `contents: write` to see drafts; a repository with assets moves its workflow ref with this verctl, or each draft waits for the next push to be published.
