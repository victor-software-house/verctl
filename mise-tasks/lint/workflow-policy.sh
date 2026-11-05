#!/bin/sh
#MISE description="The workflows follow the shared workflow policy: drafts, concurrency, labels, permissions, pins, mise_env"
# conftest pulls the policy by commit into the git directory, so the
# repository carries no copy. It reads the policy's defaults.yaml, the
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
settings=.github/workflow-policy
if [ -d "$render/.github/workflow-policy" ]; then settings="$render/.github/workflow-policy"; fi
set -- -p "$policy" --data "$policy/defaults.yaml"
if [ -d "$settings" ]; then set -- "$@" -p "$settings"; fi
if [ -f "$settings/config.yaml" ]; then set -- "$@" --data "$settings/config.yaml"; fi
if [ -f .github/actionlint.yaml ]; then set -- "$@" --data .github/actionlint.yaml; fi
git ls-files -z ':(glob).github/workflows/*.yml' ':(glob).github/workflows/*.yaml' | xargs -0 conftest test --parser ignore --update 'git::https://github.com/victor-software-house/actions.git//policy?ref=636f7f7f0babaf1bdd62856123e8fb068c265f31' "$@" --namespace workflow_drafts --namespace workflow_concurrency --namespace workflow_labels --namespace workflow_permissions --namespace workflow_pins --namespace workflow_mise_env
