# Release a new package first as 0.0.1

## Why

verctl applies whatever bump a fragment asks for. A new package at `0.0.0`
with a `minor` fragment releases `0.1.0`. One package's first release went out
as `0.1.0` that way, and its tag, its GitHub Release, and every pin to it had
to be withdrawn so it could start again at `0.0.1`. verctl 0.3.10 states the
rule in the instructions and the operator skill, but nothing enforces it.

Queue row: VER-042.

## What changes

1. `prepare` refuses a `minor` or `major` bump on a package whose version is
   exactly `0.0.0`. The error names the package, the rule, and the override.
2. `check` loads `.ctl/ver.yaml` and plans the pending fragments the way
   `prepare` does. A refused bump, or a fragment naming an undeclared package,
   fails `check` on the pull request that adds it.
3. A package declares `first_minor: true` to open at `0.1.0` on purpose. The
   key does nothing once the package has released.
4. `none` and `patch` fragments are unaffected, and the refusal of `major` on
   any 0.x package stays.

## Override

The override is `first_minor: true` on the package in `.ctl/ver.yaml`. There is
no global switch: opening past `0.0.x` is decided per package, in a file the
pull request shows. To disable the refusal for a package, set the key.

Alternatives considered:

1. A flag or an environment variable on `prepare`. It lost because the Version
   PR runs in CI, where a one-off flag is invisible in review and the decision
   would not live in the repository.
2. A `first_release: <version>` key. It lost because `major` on 0.x is already
   refused, so `0.1.0` is the only other version a first release could reach,
   and a boolean says the same thing.

## Impact

1. `check` now needs `.ctl/ver.yaml` and the manifests it names. It already
   failed on a bad fragment; it now also fails on an undeclared package, which
   `prepare` already refused.
2. A repository that wants this on its pull requests runs `check` in its verify
   task. This repository's `verify` gains a `fragments` task that runs it.
3. `bump::apply` takes the package's `first_minor`.
