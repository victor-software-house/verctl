# Design

## Context

`assets.dir` and `assets.files` set what every target's tarball holds ([package-asset-files][package-asset-files]).  
Its design weighed per-target lists and left them out, because no caller then shipped different files per platform.  
One caller now does, as the proposal shows.

## Decisions

1. **`files` on the target replaces the shared list, and `dir` stays shared.**  
   `{triple}`, `{os}`, and `{arch}` already vary the directory per target.  
   A per-target `dir` was considered and lost: no caller needs it, and a target could then pack from a directory its build never writes.
2. **The build stays shared.**  
   A build argv already takes the placeholders, such as `mise run "build:{os}"`, so each target can build what its list needs.  
   A per-target `build` was considered and lost for the same reason.
3. **Replace, not merge.**  
   A target's list is the whole tarball, so a reader sees what it holds in one place.  
   Adding to the shared list was considered and lost: it cannot drop a shared file.  
   Dropping one is the case that started this change.
4. **An unset target list means the shared one, and an empty list fails.**  
   `files: []` on a target would pack nothing, so the load refuses it.
5. **The plan resolves one pack per target.**  
   `release asset <target>` packs and checks that target's list, and a missing path names the list it came from, such as `assets.linux-x64.files`.
6. **The load problem names the target through garde's path.**  
   The targets sit flattened under `assets`, so garde names a target's field `assets.targets.<id>.files`, and the problem lands on the `assets` line.  
   A hand-written `Validate` that places it on the target's own line was considered and lost: it replaces the declared rules with hand-written traversal.

Override and disable path: omit `files` on a target to pack the shared list.

[package-asset-files]: ../2026-11-10-package-asset-files/design.md
