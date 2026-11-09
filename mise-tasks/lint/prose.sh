#!/usr/bin/env bash
#MISE description="Prose style of the Markdown docs; .vale.ini picks the files"
#MISE depends=["prose:sync"]
#MISE tools = { "aqua:vale-cli/vale" = "3.26.0" }
# Vale walks every directory it is given, ignored ones included, so a build
# writing target/ in the same verify fails the walk. It reads only the
# tracked files that still exist, and .vale.ini still picks which of them it
# checks. The list goes through a file so a git failure stops the task.
set -euo pipefail
tracked=$(mktemp)
trap 'rm -f "$tracked"' EXIT
git ls-files -z >"$tracked"
files=()
while IFS= read -r -d '' file; do
  if [ -e "$file" ]; then files+=("$file"); fi
done <"$tracked"
if [ "${#files[@]}" -gt 0 ]; then
  printf '%s\0' "${files[@]}" | xargs -0 vale
fi
