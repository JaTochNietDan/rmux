#!/bin/sh
# Read-only process snapshots. App must already be running with its test layout.
set -eu
pid="${1:?provide application PID}"
case "$pid" in ''|*[!0-9]*) echo 'PID must be numeric' >&2; exit 1 ;; esac
sample() {
    date -u '+%Y-%m-%dT%H:%M:%SZ'
    ps -p "$pid" -o pid=,ppid=,time=,rss=,command=
    # Direct shell children are separate from frontend memory/CPU.
    ps -axo pid=,ppid=,time=,rss=,command= | awk -v parent="$pid" '$2 == parent'
}
sample
sleep 30
sample
sleep 30
sample
