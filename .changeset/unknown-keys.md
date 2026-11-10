---
verctl: minor
verctl:group: changed
---

An unknown key under a `packages` entry, a `pins` entry, or a `patterns` entry now fails the load.  
Before, verctl dropped it: `publsher: none` on a Cargo package published it to crates.io.  
The error names the key and, for a package, the package.  
The driver fields a package may carry inline, such as `format` and `keys`, count as its keys.

Fix a misspelled key before moving to 0.8.0, since a config that loaded before can fail now.
