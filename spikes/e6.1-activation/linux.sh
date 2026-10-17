#!/usr/bin/env bash
# E6.1 activation spike, Linux. Throwaway: it proves what the real
# FontActivator (E6.2+) can rely on; see docs/adr/0008-font-activation.md.
#
# Activation = a symlink to the file in place under the user's font folder,
# which every fontconfig app reads. Nothing is copied. Every look runs in its
# own process (fc-list) or in a process started before the change (watch).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"

fonts="${XDG_DATA_HOME:-$HOME/.local/share}/fonts/font-manager"
session_fam="Diplomata"; session_path="$root/fixtures/fonts/google-repo/diplomata/Diplomata-Regular.ttf"
user_fam="Emblema One"; user_path="$root/fixtures/fonts/google-repo/emblemaone/EmblemaOne-Regular.ttf"

rows=()
gone() { while kill -0 "$1" 2>/dev/null; do sleep 0.2; done; }
look() { if fc-list ":family=$1" family | grep -q .; then echo true; else echo false; fi; }
row() { local r; r="$(look "$2")"; rows+=("| $1 | $r |"); printf '%-62s %s\n' "$1" "$r"; }
start_watch() {
    local ready; ready="$(mktemp -u)"
    python3 "$here/linux-watch.py" "$1" "$ready" 60 > "$ready.out" 2>&1 & echo $! > "$ready.pid"
    while [ ! -f "$ready" ]; do sleep 0.2; done; echo "$ready"
}
end_watch() { gone "$(cat "$2.pid")"; local r; r="$(cat "$2.out")"; rows+=("| $1 | $r |"); printf '%-62s %s\n' "$1" "$r"; }

echo "== $(. /etc/os-release && echo "$PRETTY_NAME"), fontconfig $(fc-list --version 2>&1 | grep -o '[0-9.]*$')"
# The app creates its folders at first start, long before it activates
# anything; a running app only watches folders that existed when it started.
mkdir -p "$fonts/session"
fc-cache -f >/dev/null

# Session: links under <fonts>/session/, removed on quit (and on the next start
# after a crash).
row "session: before" "$session_fam"
w="$(start_watch "$session_fam")"
# fontconfig compares folder times at 1 s resolution against the time it last
# loaded; a change in that same second is missed. Real apps start long before.
sleep 2
ln -sf "$session_path" "$fonts/session/"
row "session: symlink, no fc-cache, new process" "$session_fam"
fc-cache "$fonts" >/dev/null
row "session: symlink + fc-cache, new process" "$session_fam"
end_watch "session: process already running before the link" "$w"
rm -f "$fonts/session/$(basename "$session_path")"
row "session: link removed, no fc-cache, new process" "$session_fam"
fc-cache "$fonts" >/dev/null
row "session: link removed + fc-cache, new process" "$session_fam"

# Permanent: links directly under <fonts>/.
row "user: before" "$user_fam"
ln -sf "$user_path" "$fonts/"
fc-cache "$fonts" >/dev/null
row "user: symlink + fc-cache, new process" "$user_fam"
# The link must follow the file: a moved file leaves a dangling link.
moved="$(mktemp -d)/$(basename "$user_path")"; cp "$user_path" "$moved"
ln -sf "$moved" "$fonts/"; fc-cache "$fonts" >/dev/null; rm "$moved"; fc-cache "$fonts" >/dev/null
row "user: target file deleted (dangling link) + fc-cache, new process" "$user_fam"
fc-cache -f "$fonts" >/dev/null
row "user: dangling link + fc-cache -f, new process" "$user_fam"
rm -f "$fonts/$(basename "$user_path")"; fc-cache "$fonts" >/dev/null
row "user: link removed, new process" "$user_fam"

out="$here/results-linux.md"
{ echo "### $(. /etc/os-release && echo "$PRETTY_NAME")"; echo; echo "| step | result |"; echo "|---|---|"; printf '%s\n' "${rows[@]}"; } > "$out"
[ -n "${GITHUB_STEP_SUMMARY:-}" ] && cat "$out" >> "$GITHUB_STEP_SUMMARY"
exit 0
