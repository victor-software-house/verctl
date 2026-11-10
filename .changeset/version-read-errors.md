---
verctl: patch
verctl:group: fixed
---

`release`, `render`, `release asset`, and `check --versions` read a package's version the way `version` does.  
On a failed read, the error names the package and its file.  
`check --versions` fails on a declared package whose file is missing, where it left the package out of the report.
