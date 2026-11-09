#!/bin/sh
#MISE description="Fail when a mise lockfile no longer matches its config"
# pre-push runs this. `mise lock --dry-run` writes and installs nothing, and
# prints [] when every lock is current; `mise install --locked` would download
# a missing tool instead.
#
# A dry run checks only the locks of the profiles MISE_ENV loads, so it runs
# once for each committed mise.<profile>.toml, which also checks mise.lock;
# a MISE_ENV the pusher's shell exports changes nothing. Under the hooks
# profile, mise would assign the pinned catalogs' task tools to that
# profile's lockfile, which nothing writes, so it is left out, and so is a
# machine's own mise.local.toml. Only tracked files count, so an untracked
# profile the push does not carry cannot fail it. hooks pieces always render
# mise.dev.toml.
configs=$(git ls-files -- 'mise.*.toml') || { echo "✘ git ls-files failed; see its error above"; exit 1; }
stale=0
for config in $configs; do
  profile=${config#mise.}
  profile=${profile%.toml}
  case $profile in
    hooks | local | *.local) continue ;;
  esac
  changes=$(MISE_ENV=$profile mise lock --dry-run --json) || { echo "✘ mise lock --dry-run failed in the $profile profile; see its error above"; exit 1; }
  if [ "$changes" != "[]" ]; then
    printf '%s\n' "$changes"
    echo "✘ a mise lockfile of the $profile profile is stale: run MISE_ENV=$profile mise lock"
    stale=1
  fi
done
exit "$stale"
