#!/usr/bin/env sh
# System libraries Tauri needs to build NearClip on Debian and Ubuntu.
# Used by CI (.github/workflows/build.yml) and by contributors on Linux.
set -eu

sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libxdo-dev
