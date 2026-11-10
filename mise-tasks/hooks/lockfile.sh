#!/bin/sh
#MISE description="Fail when a mise lockfile no longer matches its config"
# pre-push runs this. It runs `mise lock` for the lockfile_platforms entries
# and fails when a lockfile then differs from HEAD, so the pusher reviews and
# commits the change. `mise lock --dry-run` reports only version changes and
# misses a missing platform entry or download URL, which the real lock
# writes. The real lock also leaves out what no machine installs: a platform
# a tool's `os` option excludes, and one its release ships no build for. It
# runs against an empty global and system config directory, because a tool
# the pusher's own config pins has no entry in any project lock; the
# repository is trusted by path instead, since the pusher's
# trusted_config_paths is dropped with that config.
#
# `mise lock` writes only the locks of the profiles MISE_ENV loads, so it
# runs once for each committed mise.<profile>.toml, which also locks
# mise.toml; a MISE_ENV the pusher's shell exports changes nothing. Under the
# hooks profile, mise would assign the pinned catalogs' task tools to that
# profile's lockfile, which nothing writes, so it is left out, and so is a
# machine's own mise.local.toml. Only tracked files count, so an untracked
# profile the push does not carry cannot fail it. hooks pieces always render
# mise.dev.toml.
platforms=$(mise settings get lockfile_platforms) || { echo "✘ mise settings get lockfile_platforms failed; see its error above"; exit 1; }
platforms=$(printf '%s' "$platforms" | tr -d '[]" ')
[ -n "$platforms" ] || { echo "✘ lockfile_platforms is empty; list the platforms the locks record in mise.toml"; exit 1; }
isolated_config=$(mktemp -d) || { echo "✘ mktemp failed; see its error above"; exit 1; }
trap 'rm -rf "$isolated_config"' EXIT
isolated() { env MISE_CONFIG_DIR="$isolated_config" MISE_SYSTEM_CONFIG_DIR="$isolated_config" MISE_TRUSTED_CONFIG_PATHS="$PWD" "$@"; }
configs=$(git ls-files -- 'mise.*.toml') || { echo "✘ git ls-files failed; see its error above"; exit 1; }
# The locks of the profiles locked below; no other lock file can fail the push.
locked=mise.lock
for config in $configs; do
  profile=${config#mise.}
  profile=${profile%.toml}
  case $profile in
    hooks | local | *.local) continue ;;
  esac
  # mise lock reports progress even with --quiet, so its output shows only
  # when it fails.
  if ! output=$(isolated MISE_ENV="$profile" mise lock --platform "$platforms" 2>&1); then
    printf '%s\n' "$output"
    echo "✘ mise lock failed in the $profile profile; see its error above"
    exit 1
  fi
  locked="$locked mise.$profile.lock"
done
# HEAD is what the push carries, so a lock fixed on disk but not committed
# still fails, and a profile's first lock shows as untracked. The lock of a
# machine's own mise.local.toml, which mise lock also writes, is not listed.
# shellcheck disable=SC2086 # one lock file name per word
changed=$(git status --porcelain --untracked-files=all -- $locked) || { echo "✘ git status failed; see its error above"; exit 1; }
if [ -n "$changed" ]; then
  printf '%s\n' "$changed"
  echo "✘ these lockfiles differ from HEAD after mise lock: review the change and commit it"
  exit 1
fi
