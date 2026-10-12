# Design

## Decisions

1. **GitHub auto-merge, not a merge step that waits.** The action enables
   auto-merge and exits; GitHub merges when the required checks pass.
   Alternative: poll the checks inside the workflow and merge. Lost: it holds
   a runner for the whole CI run and duplicates what branch rules already
   enforce.
2. **Refuse without a required check.** The action reads the base branch's
   rules through `gh api repos/<repo>/rules/branches/<base>` and stops unless
   one is `required_status_checks`. Alternative: trust the repository
   settings. Lost: with no required check, GitHub merges immediately, so a
   red Version PR would ship.
3. **Squash message from the PR.** Subject `<title> (#<number>)`, body the PR
   description, as this workspace merges every pull request. Alternative: the
   repository's default squash message. Lost: it differs per repository.
4. **Refuse `GITHUB_TOKEN`.** GitHub merges as whoever enabled auto-merge,
   and an event made with `GITHUB_TOKEN` starts no workflow run
   ([GitHub docs][token-events]), so the merge would never publish. The
   action fails unless `token` is another token, such as a GitHub App token.
   That token also makes the Version PR itself start CI, which the required
   check needs. Alternative: document it only. Lost: the failure is silent,
   a merged release that never publishes.

## Override and disable

`auto-merge` defaults to `false`. A repository disables it by removing the
input or setting it to `false`. An open Version PR's auto-merge can be
cancelled with `gh pr merge --disable-auto <number>`.

[token-events]: https://docs.github.com/en/actions/concepts/security/github_token#when-github_token-triggers-workflow-runs
