# Tasks

- [ ] 1.1 `assets.files` loads, and a duplicate file name or a path with no file name fails the load; proof: the `assets.files` config tests pass in CI
- [ ] 1.2 Release lookups list drafts and count only `uploaded` assets; proof: the `github` draft lookup tests pass in CI against a mocked API
- [ ] 1.3 `publish` creates the tag, then a draft, for a tag with declared assets, and publishes the draft once every declared asset is uploaded; proof: the `github` draft and finalize tests pass in CI
- [ ] 1.4 A draft Release is incomplete in `pending`; proof: the `pending` draft test passes in CI
- [ ] 1.5 `assets --upload --file` uploads a declared file and refuses an undeclared one; proof: the `assets` file tests pass in CI
- [ ] 1.6 The reusable workflow finalizes after `assets`; proof: the next verctl release goes from draft to published with both tarballs
- [ ] 1.7 Operator docs describe drafts and `assets.files`; proof: `operator_docs` passes in CI
