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
always the same command, `verctl render` (§5).

Public override: none. A served file that should not be checked is not a
served file; delete or ignore its template. Disable path: the same.

## 3. Pins hold the current version

For each `pins` entry, `check` runs the pin rewrite with the versions on
HEAD in memory.
It fails when a pattern no longer matches at its declared count.
It also fails when the rewrite would change the file, which means the file
names another version.

## 4. One owner per file

A template's target is known only after the template renders, so the
refusal runs wherever templates render: `version` before any write, `check`,
and `render`. `version` cannot render over a pin. `status` reads no config
and is not covered. The message names the target, the template path, and
the `pins` entry by its file and package.

Alternative considered: refuse the overlap in `Config::load`. It lost
because loading would have to render every template, and a render is the only
source of a target.

Alternative considered: let the template win and warn. It lost because a
warning in a release run is read by no one, and the pin's edit is lost.

## 5. `render` is the fix

`render` rewrites the `pins` entries, then renders every template at the
versions on HEAD through `templates::write`. It plans both first, so a
pin that no longer matches, a template that fails, or a file with two
owners writes nothing.

Without it, `version` renders templates only when a fragment bumps a package.
A pull request that edits a template would then have no command that makes
`check` pass.

The lead placed this in 0.8.0 as an operator-scope call on 2026-11-10,
because `render` already breaks its callers in that minor. Rendering
through `ctl_core::served` stays a later change.

## 6. Cost

`check` now reads `.ctl/ver.yaml` even with no fragments. Past that, a
repository with no templates and no pins pays nothing: `templates::any`
already returns early, and an empty `pins` list does no work. A repository
with templates reads every manifest once, as `version` does.
