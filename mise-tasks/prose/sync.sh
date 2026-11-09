#!/usr/bin/env bash
#MISE description="Fetch the Vale packages .vale.ini names into .vale/styles"
#MISE tools = { "aqua:vale-cli/vale" = "3.26.0" }
#MISE sources=[".vale.ini"]
#MISE outputs=[".vale/styles/.synced"]
# A failed sync can leave some packages in place, newer than .vale.ini. The
# output is therefore a stamp written only after vale sync succeeds, and until
# then mise runs the sync again.
set -euo pipefail
vale sync
touch .vale/styles/.synced
