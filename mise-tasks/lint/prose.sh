#!/usr/bin/env bash
#MISE description="Prose style of the Markdown docs; .vale.ini picks the files"
#MISE depends=["prose:sync"]
#MISE tools = { "aqua:vale-cli/vale" = "3.26.0" }
# Vale walks every directory it is given, ignored ones included, so a build
# writing target/ in the same verify fails the walk. It reads only the
# tracked files that are still regular files, and .vale.ini still picks
# which of them it checks. A tracked link to a directory is left out: Vale
# walks it, and through a link to the repository root, such as a benchmark
# package's path dependency, it descends into the repository again until the
# path is too long and fails with "unable to resolve symlink". The list goes
# through a file so a git failure stops the task. With files as arguments,
# the task reads those instead of the tracked ones, and .vale.ini still picks
# which of them it checks.
set -euo pipefail
tracked=$(mktemp)
trap 'rm -f "$tracked"' EXIT
if [ "$#" -gt 0 ]; then
  printf '%s\0' "$@" >"$tracked"
else
  git ls-files -z >"$tracked"
fi
files=()
while IFS= read -r -d '' file; do
  if [ -f "$file" ]; then files+=("$file"); fi
done <"$tracked"
if [ "${#files[@]}" -gt 0 ]; then
  printf '%s\0' "${files[@]}" | xargs -0 vale
fi
