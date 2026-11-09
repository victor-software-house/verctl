# Design

## 1. Versions on HEAD

`check` renders with the version each package's manifest holds in the
working tree, the same map `version` builds for templates when no fragment
bumps anything. On a feature branch those are the released versions, and on
the Version PR they are the new ones `version` wrote. Either way a served
file that `version` would leave alone is one `check` accepts.

Alternative considered: render at the versions of the last release tags. It
lost because the Version PR moves versions before any tag exists, so every
Version PR would fail.

## 2. Compare bytes, report every file

Each template renders in memory and is compared byte for byte with its
target. `check` collects every mismatch before failing, so one run lists
every file to fix. Each line names the target and its template. The fix is
always the same: edit the template, then run `verctl version`, or revert the
hand edit.

Public override: none. A served file that should not be checked is not a
served file; delete or ignore its template. Disable path: the same.

## 3. Pins hold the current version

For each `pins` entry, `check` runs the pin rewrite with the versions on
HEAD in memory. It fails when a pattern no longer matches at its declared
count, and when the rewrite would change the file, which means the file names
another version.

## 4. One owner per file

The overlap is a property of the declarations, not of a run, so
`Config::load` refuses it. That covers every verb, and `version` cannot
render over a pin. A template's target is known only after its file is read,
so the check runs where templates are listed. The message names the target,
the template path, and the `pins` entry by its file.

Alternative considered: let the template win and warn. It lost because a
warning in a release run is read by no one, and the pin's edit is lost.

## 5. Cost

A repository with no templates and no pins pays nothing: `templates::any`
already returns early, and an empty `pins` list does no work. A repository
with templates reads every manifest once, as `version` does.
