---
verctl: patch
---

`docs/served-files.md` §4 no longer promises that a prerelease always fails the
release, because it does not. The condition is exact: a match ending in
literal text has a right-hand boundary, so `verctl@{version} today` reads
`verctl@1.0.0-rc.1 today` as no match at all. A match ending *at* the
placeholder has none, and rewrites the numeric head, leaving the tail.

The doc now says that, and says what to do about it — put the following text
into `match`, or use `whole_line` for a line the match owns. It also records
why nothing can be inferred: `download/v{version}` against
`download/v1.2.3-linux-x64.tar.gz` is correct and pins 1.2.3, so a rule
refusing every trailing `-` would stop real releases.
