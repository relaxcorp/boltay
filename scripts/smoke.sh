#!/usr/bin/env bash
# Starts the app on a virtual display and lets it run for 10 s: it passes if timeout had to
# stop it and nothing panicked. Usage: scripts/smoke.sh <command> [args...]
set -u
log=$(mktemp)
timeout -k 5 10 xvfb-run -a "$@" >"$log" 2>&1
code=$?
cat "$log"
if [ "$code" -ne 124 ]; then
  echo "exited with $code before the timeout"
  exit 1
fi
if grep -q panicked "$log"; then
  echo "panicked"
  exit 1
fi
echo "ran for 10 s"
