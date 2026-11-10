---
verctl: minor
verctl:group: changed
---

A publisher or query argument with a `{placeholder}` that has no value fails and names the placeholder.  
Before, verctl left that argument off the command line.  
A typo such as `{registr}` fails this way, and so does `{registry}` on a package with no registry.  
Braces that are not a placeholder name, such as a JSON argument, pass through.
