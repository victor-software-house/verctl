# Design

## Context

verctl 0.4.5 and qctl 0.5.4 both shipped with no docs.rs build, and nothing
noticed. This change adds `verctl docs` and the workflow job that runs it. It
is one of the parts [reusable-release-workflow][rrw] was split into.

## 1. The docs.rs build

`verctl docs` takes the packages released at HEAD that meet two conditions:

- the publish target is crates.io ([registry-publish-check][rpc]);
- the package has a library target as Cargo resolves it.
  That is an explicit `[lib]` table, or `src/lib.rs` found by target
  auto-discovery unless `package.autolib` is `false`.

docs.rs documents no binary-only crate.
verctl applies Cargo's documented rule itself instead of running
`cargo metadata`, so the `docs` job needs no Cargo.

It checks each such package only when the tag that `tags.template` names for
the package's version at HEAD names HEAD on GitHub.
That tag is the shared `v{version}` tag, or the package's own tag when the
template has `{name}`.

verctl reads the tag through the GitHub API, peeling an annotated tag to its
commit, and never from local refs.
Three facts explain that choice:

- the `docs` job's checkout fetched no tags;
- the `publish` job pushed the tag from another runner;
- the API needs no git credentials in the checkout.

`verctl publish` creates tags only after the registry lists every version
([registry-publish-check][rpc]).
So the tag is the record that the publish happened, and no read of a lagging
index decides it.

A crate that reached crates.io while another package failed has no tag yet.
Its commit stays incomplete, and the rerun that completes it tags the crate and
checks it then.

verctl polls `https://docs.rs/crate/<name>/<version>/status.json` with a
bounded backoff.
docs.rs answers 404 until its build of the version starts, then
`{"doc_status": <bool>, "version": "<version>"}`.
`doc_status` is false while the build runs and after it failed.
The endpoint does not distinguish the two.

verctl polls until every crate reports `doc_status` true or `--timeout`
(minutes, default 60, `VERCTL_TIMEOUT`) runs out.
It then reports each crate as `built`, `failed` (still false), or `unbuilt`
(still 404).
It exits non-zero naming every crate that is not built.
A failed build therefore fails the job when the timeout runs out, which can be
later than the moment docs.rs records it.

docs.rs can queue a build for a long time, so the check is a job of its own.
A slow queue never delays the next version commit's publish or the Version PR.

The docs.rs build does not count toward completeness ([release-at-version-commit][ravc]).

A failed build cannot be retried for the same version.
A rerun reports `nothing to release`, and the fix is the next patch release.
The failed `docs` job is the visible record.

[rrw]: ../reusable-release-workflow/design.md
[rpc]: ../registry-publish-check/design.md
[ravc]: ../release-at-version-commit/design.md
