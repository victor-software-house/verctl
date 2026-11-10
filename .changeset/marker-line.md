---
verctl: patch
verctl:group: added
---

`marker: true` on a package makes any file a package, such as one reusable workflow.  
Its version is the file's one marker line: `# version: 1.2.0`, `// version: 1.2.0`, or `<!-- version: 1.2.0 -->`.  
`version` rewrites only that version.  
A file with no marker line, or two, fails and names the line numbers.  
A marker package takes no driver, and with no publisher it releases on GitHub only.
