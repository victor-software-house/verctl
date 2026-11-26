#!/usr/bin/env bash
#MISE description="Prose style of the docs and code comments; .ctl/hakkiri.yaml picks the files"
#MISE tools = { "github:victor-software-house/hakkiri" = "0.1.2" }
# With no arguments, hakkiri walks the repository and skips what git ignores.
# With files as arguments, such as the pre-push hook passes, it reads only those.
# It exits 1 when an alert reaches the fail_level of the config.
set -euo pipefail
hakkiri check "$@"
