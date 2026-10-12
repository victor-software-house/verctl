#!/usr/bin/env bash
# Enable GitHub auto-merge on the open Version PR, as a squash titled after it.
# Refuses when the base branch has no required status check, because GitHub
# would then merge at once with no gate.
# Needs GH_TOKEN, REPO (owner/name), and BASE (the default branch).
set -euo pipefail
checks=$(gh api "repos/${REPO}/rules/branches/${BASE}" --jq '[.[] | select(.type == "required_status_checks")] | length')
if [ "${checks}" -eq 0 ]; then
  echo "::error::${BASE} has no required status check; auto-merge would merge the Version PR at once"
  exit 1
fi
number=$(gh pr list --repo "${REPO}" --head version-packages --base "${BASE}" --state open --json number --jq '.[0].number // empty')
if [ -z "${number}" ]; then
  exit 0
fi
title=$(gh pr view "${number}" --repo "${REPO}" --json title --jq .title)
body=$(mktemp)
gh pr view "${number}" --repo "${REPO}" --json body --jq .body > "${body}"
gh pr merge "${number}" --repo "${REPO}" --auto --squash --subject "${title} (#${number})" --body-file "${body}"
