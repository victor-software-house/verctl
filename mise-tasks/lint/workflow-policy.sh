#!/bin/sh
#MISE description="The workflows follow the shared workflow policy: drafts, concurrency, labels, permissions, pins, mise_env"
#MISE tools = { conftest = "0.71.1" }
# conftest pulls the policy by commit into the git directory, so the
# repository carries no copy; --absolute-paths keeps a linked worktree's
# absolute git directory whole. conftest parses each workflow as YAML, and the
# policy's texts.sh writes the workflows' text, which the pins rule reads for
# comments, as data beside it. It reads the policy's defaults.yaml, the
# repository's parameters and exceptions (.github/workflow-policy/config.yaml),
# and .github/actionlint.yaml's runner labels. Each --namespace is a rule the
# workflow_policy answer chose, or one of the repository's own packages in
# .github/workflow-policy/ that workflow_policy_namespaces names.
# GitHub runs only the repository root's workflows, so, as lint:workflows
# does, the task checks the root's workflows with the root's actionlint.yaml.
# A render below the root, such as cli/, reads its own .github/workflow-policy/
# when it keeps one, and the root's otherwise; each file is passed only where
# it exists, and a named package that no file defines fails the task. conftest
# fails on a missing folder, so it checks the tracked workflows only, and with
# none nothing runs and the check passes. -z hands xargs each path unquoted,
# whatever it holds.
render=$(pwd)
cd "$(git rev-parse --show-toplevel)" || exit 1
files=$(git ls-files ':(glob).github/workflows/*.yml' ':(glob).github/workflows/*.yaml') || exit 1
[ -n "$files" ] || exit 0
policy=$(git rev-parse --git-path workflow-policy) || exit 1
texts=$(git rev-parse --git-path workflow-texts.yaml) || exit 1
conftest pull --absolute-paths -p "$policy" 'git::https://github.com/victor-software-house/actions.git//policy?ref=ca84425f5597bc5fd59e5614d0cf1f2f06194377' || exit 1
sh "$policy/texts.sh" >"$texts" || exit 1
settings=.github/workflow-policy
if [ -d "$render/.github/workflow-policy" ]; then settings="$render/.github/workflow-policy"; fi
set -- -p "$policy" --data "$policy/defaults.yaml" --data "$texts"
if [ -d "$settings" ]; then set -- "$@" -p "$settings"; fi
if [ -f "$settings/config.yaml" ]; then set -- "$@" --data "$settings/config.yaml"; fi
if [ -f .github/actionlint.yaml ]; then set -- "$@" --data .github/actionlint.yaml; fi
git ls-files -z ':(glob).github/workflows/*.yml' ':(glob).github/workflows/*.yaml' | xargs -0 conftest test "$@" --namespace workflow_drafts --namespace workflow_concurrency --namespace workflow_labels --namespace workflow_permissions --namespace workflow_pins --namespace workflow_mise_env
