---
verctl: patch
verctl:group: added
---

A package whose `path` is `go.mod` needs no driver.  
Its version is a `// vX.Y.Z` comment at the end of the `module` line, and a module with no such comment starts at `0.0.0`.  
`version` refuses a bump to v2 or later until the module path ends in `/v<major>`, because Go reads such a module only there; a dry run refuses too.  
With no publisher, a Go module releases on GitHub only, as `publisher: none` does, since Go fetches a module from its tag.
