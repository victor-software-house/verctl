---
verctl: patch
verctl:group: added
---

A package whose `path` is `pyproject.toml` needs no driver.  
`version` rewrites `project.version` and keeps every other byte.  
When the repository holds a `uv.lock`, `version` names `uv lock` as the follow-up.  
A `pyproject.toml` that lists `version` in `project.dynamic` fails with the fixes, because its build backend sets the version.  
verctl has no stock Python publisher, so such a package names its `publisher`, or `none` to release on GitHub only.

A `pyproject.toml` package fails when `.ctl/ver.yaml` loads unless it names a `publisher`, a `publish` command, or a `publishers` entry of its own.  
That holds for a package with its own driver too.
