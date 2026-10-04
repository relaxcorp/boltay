#!/usr/bin/env bash
# System packages to build the desktop app on Ubuntu 22.04+ / Debian 12+. With --runtime,
# only what the built app needs to start, plus a virtual display for a smoke test.
set -euo pipefail

sudo=
[ "$(id -u)" -ne 0 ] && sudo=sudo

$sudo apt-get update

if [ "${1:-}" = --runtime ]; then
  # Renamed in Ubuntu 24.04 and Debian 13, the old name is only a virtual package there.
  asound=libasound2
  apt-cache show libasound2t64 >/dev/null 2>&1 && asound=libasound2t64
  packages=(
    libwebkit2gtk-4.1-0
    libgtk-3-0
    libayatana-appindicator3-1
    librsvg2-common
    libxdo3
    "$asound"
    ca-certificates
    xvfb
    xauth
  )
else
  packages=(
    build-essential
    curl
    file
    pkg-config
    libwebkit2gtk-4.1-dev
    libgtk-3-dev
    libayatana-appindicator3-dev
    librsvg2-dev
    libssl-dev
    libxdo-dev
    libasound2-dev
  )
fi

$sudo env DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends "${packages[@]}"
