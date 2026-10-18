#!/usr/bin/env bash
# E6.1 activation spike, macOS driver. Every register, look and remove runs in
# its own process, so what is measured is what a second app sees. Background
# processes start inside $(...) subshells, so they are not our children: wait by
# polling the pid, not with `wait`.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
bin="$(mktemp -d)/e61"
swiftc -O "$here/macos.swift" -o "$bin"

session_fam="Diplomata"; session_path="$root/fixtures/fonts/google-repo/diplomata/Diplomata-Regular.ttf"
user_fam="Emblema One"; user_path="$root/fixtures/fonts/google-repo/emblemaone/EmblemaOne-Regular.ttf"
proc_fam="Cagliostro"; proc_path="$root/fixtures/fonts/google-repo/cagliostro/Cagliostro-Regular.ttf"

rows=()
gone() { while kill -0 "$1" 2>/dev/null; do sleep 0.2; done; }
row() { local r; r="$("$bin" check "$2")"; rows+=("| $1 | $r |"); printf '%-62s %s\n' "$1" "$r"; }
start_watch() { local ready; ready="$(mktemp -u)"; "$bin" watch "$1" "$ready" 20 > "$ready.out" 2>/dev/null & echo $! > "$ready.pid"; while [ ! -f "$ready" ]; do sleep 0.2; done; echo "$ready"; }
end_watch() { gone "$(cat "$2.pid")"; local r; r="$(cat "$2.out")"; rows+=("| $1 | $r |"); printf '%-62s %s\n' "$1" "$r"; }
start_reg() { local ready; ready="$(mktemp -u)"; "$bin" register "$1" "$2" "$ready" >/dev/null 2>&1 & echo $! > "$ready.pid"; while [ ! -f "$ready" ]; do sleep 0.2; done; echo "   register $1: $(cat "$ready")" >&2; echo "$ready"; }
end_reg() { rm -f "$1"; gone "$(cat "$1.pid")"; sleep 1; }

echo "== macOS $(sw_vers -productVersion) ($(sw_vers -buildVersion))"

# Control: process scope must stay inside the registering process.
row "process: before" "$proc_fam"
r="$(start_reg process "$proc_path")"
row "process: registrar alive, other process" "$proc_fam"
end_reg "$r"

# Session scope.
row "session: before" "$session_fam"
w="$(start_watch "$session_fam")"
r="$(start_reg session "$session_path")"
row "session: registrar alive, new process" "$session_fam"
end_watch "session: process already running before registration" "$w"
end_reg "$r"
row "session: registrar exited, new process" "$session_fam"
echo "   unregister: $("$bin" unregister session "$session_path")"
row "session: after unregister (other process)" "$session_fam"

# User (persistent) scope.
row "user: before" "$user_fam"
w="$(start_watch "$user_fam")"
r="$(start_reg user "$user_path")"
row "user: registrar alive, new process" "$user_fam"
end_watch "user: process already running before registration" "$w"
end_reg "$r"
row "user: registrar exited, new process" "$user_fam"
echo "   unregister: $("$bin" unregister user "$user_path")"
row "user: after unregister (other process)" "$user_fam"

out="$here/results-macos-$(sw_vers -productVersion).md"
{ echo "### macOS $(sw_vers -productVersion) ($(sw_vers -buildVersion))"; echo; echo "| step | result |"; echo "|---|---|"; printf '%s\n' "${rows[@]}"; } > "$out"
[ -n "${GITHUB_STEP_SUMMARY:-}" ] && cat "$out" >> "$GITHUB_STEP_SUMMARY"
exit 0
