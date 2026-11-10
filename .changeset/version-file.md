---
verctl: patch
verctl:group: added
---

A package whose `path` is `VERSION` needs no driver: the file holds one version and a newline, and anything else in it fails.  
A missing `VERSION` file reads as `0.0.0`, so a new package needs no file before its first release.  
`check` leaves the file missing, and `version` writes it.

A `VERSION` path whose directory is missing fails the plan before `version` writes anything.  
With no publisher, a `VERSION` package releases on GitHub only, as `publisher: none` does.
