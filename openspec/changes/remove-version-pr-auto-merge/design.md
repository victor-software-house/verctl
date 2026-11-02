# Design

## 1. Remove the feature, do not default it off

The input already defaulted to `false`.
Keeping the flag with a default of off would leave a path that arms a release from CI, which the operator ruled out.
Removing it makes a caller that still passes `auto-merge: true` fail loudly when it moves to this release, so no caller keeps it by accident.

Alternative: keep `auto-merge` as a deprecated input that does nothing.
It lost because a silent no-op hides the caller's stale setting, and the operator asked for removal.

## 2. Drop the release hold with it

verctl's own `self-release.yml` had a `hold` job and a `VERCTL_RELEASE_HOLD` variable that turned auto-merge off on the open Version PR.
Without auto-merge, every Version PR already waits for a person, so the job has nothing to do.

Alternative: keep the hold job for a future need.
It lost because it only calls `gh pr merge --disable-auto`, which has no effect once nothing enables auto-merge.

## 3. Override and disable path

The change removes an automatic behaviour rather than adding one.
The Version PR's merge is the person's action, and nothing in verctl overrides it.
