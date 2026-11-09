#!/usr/bin/env bash
#MISE description="Prose style of the Markdown docs; .vale.ini picks the files"
#MISE depends=["prose:sync"]
#MISE tools = { "aqua:vale-cli/vale" = "3.26.0" }
vale .
