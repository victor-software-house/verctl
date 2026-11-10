# Design

## Context

`assets` builds one tarball per target from the first package's recipe.
The recipe names one `binary`, and the tarball holds that file under the name `bin`.

## Decisions

1. **`dir` and `files` sit on the shared recipe, beside `binary`.**
   Every target packs the same list, and `{triple}`, `{os}`, and `{arch}` vary the directory per target.
   Per-target lists were considered and lost: no caller ships different files per platform, and the recipe already holds every other packing choice.
2. **The same two keys as `vendored.<name>`.**
   A repository that writes both sections reads one shape.
   A nested `pack:` mapping was considered and lost for that reason.
3. **`binary` may not be set beside them.**
   It would never be read, and a reader would take it for what the tarball holds.
   The load names `binary` and says to list the binary in `files`.
4. **A file the build does not write is staged by the build.**
   verctl packs what exists under `dir` and does not copy files in.
   A copy list was considered and lost: the repository's `build` argv already runs its own steps.

Override and disable path: omit `dir` and `files` to get the single-binary tarball.
