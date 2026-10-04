#!/usr/bin/env bash
# The official ONNX Runtime for Linux, linked dynamically and shipped next to the app. It
# needs glibc 2.27, while the prebuilt static one needs 2.38 and would not run on 22.04.
# Usage: scripts/fetch-ort.sh [dir], target/onnxruntime by default; prints the lib folder.
set -euo pipefail

version=1.28.0
sha256=a3e1b79d7bb1bf09696ce675f49e4064e6c81f6202b8225624fff0e93f8d6407
repo=$(cd "$(dirname "$0")/.." && pwd)
dir=${1:-$repo/target/onnxruntime}

if [ ! -f "$dir/lib/libonnxruntime.so.$version" ]; then
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT
  curl -fsSL -o "$tmp/ort.tgz" \
    "https://github.com/microsoft/onnxruntime/releases/download/v$version/onnxruntime-linux-x64-$version.tgz"
  echo "$sha256  $tmp/ort.tgz" | sha256sum -c --quiet
  mkdir -p "$dir"
  tar xzf "$tmp/ort.tgz" -C "$dir" --strip-components=1
fi
echo "$dir/lib"
