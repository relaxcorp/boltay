#!/usr/bin/env bash
set -euo pipefail
# The API rather than browser_download_url, so GITHUB_TOKEN also works for a private fork.

repo=${BOLTAY_REPO:-relaxcorp/boltay}
tag=${1:-models-v1}
out=${2:-models}
api=https://api.github.com/repos/$repo

auth=()
[ -n "${GITHUB_TOKEN:-}" ] && auth=(-H "Authorization: Bearer $GITHUB_TOKEN")

mkdir -p "$out"
curl -fsSL "${auth[@]}" "$api/releases/tags/$tag" |
  jq -r '.assets[] | "\(.id) \(.name)"' |
  while read -r id name; do
    case $name in
      *__*) dir=$out/${name%%__*}; file=${name#*__} ;;
      *) dir=$out; file=$name ;;
    esac
    mkdir -p "$dir"
    # The checksum list changes with every new model, the rest never does.
    [ "$name" != SHA256SUMS ] && [ -s "$dir/$file" ] && continue
    echo "$name"
    curl -fsSL "${auth[@]}" -H "Accept: application/octet-stream" \
      -o "$dir/$file.part" "$api/releases/assets/$id"
    mv "$dir/$file.part" "$dir/$file"
  done

cd "$out"
while read -r sum name; do
  case $name in
    *__*) path=${name%%__*}/${name#*__} ;;
    *) path=$name ;;
  esac
  [ -f "$path" ] && echo "$sum  $path"
done < SHA256SUMS | sha256sum -c --quiet
echo ok
