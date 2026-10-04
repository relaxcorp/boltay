#!/usr/bin/env bash
# Regenerates THIRD_PARTY_NOTICES.md. Needs cargo-about 0.9.2 (cargo install cargo-about
# --version 0.9.2 --locked), app/node_modules (npm ci in app) and the network once, for the
# ONNX Runtime notices.
set -euo pipefail
repo=$(cd "$(dirname "$0")/.." && pwd)
here=$repo/scripts/notices
out=$repo/THIRD_PARTY_NOTICES.md
ort=$(dirname "$("$repo/scripts/fetch-ort.sh")")

{
  printf '# Third-party notices\n\n'
  printf 'Boltay is built on the work of others. Their licenses follow.\n\n'
  cat "$here/models.md"
  printf '## ONNX Runtime\n\n'
  printf 'The speech runtime by Microsoft, %s. It is linked into the app on Windows and\n' "$(cat "$ort/VERSION_NUMBER")"
  printf 'macOS and ships next to it on Linux. Its license and the notices of the code it is\n'
  printf 'built from follow.\n\n'
  printf '```text\n%s\n```\n\n' "$(cat "$ort/LICENSE")"
  printf '```text\n%s\n```\n\n' "$(tr -d '\r' < "$ort/ThirdPartyNotices.txt")"
  node "$here/npm.mjs" "$repo/app"
  cargo about generate --config "$here/about.toml" --manifest-path "$repo/Cargo.toml" \
    --workspace "$here/about.hbs"
} > "$out"
echo "$out"
