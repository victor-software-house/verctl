---
verctl: minor
verctl:group: changed
---

`verctl release` reads the registry token from `VERCTL_REGISTRY_TOKEN` or `VERCTL_TRUSTED_PUBLISHING_TOKEN`, and fails when both are set.  
It gives the token to each publisher and its registry `query` under `--registry-token-env` (`VERCTL_REGISTRY_TOKEN_ENV`, default `CARGO_REGISTRY_TOKEN`).  
The queries that decide whether a release is complete or a version was skipped get it too.  
No other command it starts gets either variable, and no flag takes the token.

No command verctl starts inherits `GITHUB_TOKEN` or `GH_TOKEN` either: not a publisher, not a build script it runs, not `assets.prepare` or `assets.build`, and not `prepare.after`.  
Before, each ran with the job's token, which can write contents.  
verctl keeps the token for its own GitHub calls, and its own `git push` takes it through a private variable.  

A publisher that reads `GITHUB_TOKEN` gets the registry token there when `registry-token-env` is `GITHUB_TOKEN`.  
A bunfig for GitHub Packages with `token = "$GITHUB_TOKEN"` is one.

The reusable `release.yml` sets these in its publish step and runs `mise run ver release`.  
Its registry-token inputs and secrets keep their names.  
Only its `plan` job checks that the pinned verctl is 0.8.0 or later.

A pending version commit that pins an older verctl fails on the `release` verb.  
A merged commit's pin cannot move, so finish every pending release before moving the workflow ref to 0.8.0.

To release such a commit later, check it out and run `verctl release` from a 0.8.0 install there.

Every job of `release.yml` now sets up mise with `victor-software-house/actions/mise`, which downloads tools through the organization's release mirror.  
Callers already grant `id-token: write` for the publish job, and that grant now serves the mirror too.  
A job without it downloads from GitHub and logs a notice.

Every job of `release.yml` has a `timeout-minutes`: 10 for `plan` and `propose`, 30 for `publish`, 45 for `assets`, and 75 for `docs`.  
A publish that runs longer than 30 minutes is cancelled.
