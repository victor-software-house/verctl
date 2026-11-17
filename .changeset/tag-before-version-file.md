---
"verctl": patch
verctl:group: fixed
---

A release tag on a commit that holds no file at the package's path, such as one made before the version moved to a `VERSION` file, now counts as that version's release when HEAD's history holds it.
`status` and `release` no longer fail on a repository that moved its version source after a release.
