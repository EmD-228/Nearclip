#!/usr/bin/env sh
# Regenerates docs/screenshots/nearclip-{light,dark}.webp: the desktop History
# view next to the phone Devices view, rendered from the built frontend with
# mock-tauri.js standing in for the Rust backend.
#
# Needs Playwright's Chromium (fetched by npx on first run), ImageMagick and cwebp.
# Run from the repository root: sh scripts/screenshots/capture.sh
set -eu

PORT=8790
OUT=docs/screenshots
WORK=$(mktemp -d)
trap 'kill "$SERVER" 2>/dev/null; rm -rf "$WORK"' EXIT

pnpm build
cp -R dist/. "$WORK/"
cp scripts/screenshots/mock-tauri.js "$WORK/"
# Load the mock before the app bundle.
sed 's|<head>|<head><script src="/mock-tauri.js"></script>|' dist/index.html > "$WORK/index.html"

python3 -m http.server "$PORT" --bind 127.0.0.1 --directory "$WORK" >/dev/null 2>&1 &
SERVER=$!
sleep 1

shoot() { # name platform view device width,height scheme
  npx -y playwright@1.61 screenshot --browser=chromium --device="$4" --viewport-size="$5" \
    --color-scheme="$6" --wait-for-timeout=1500 \
    "http://127.0.0.1:$PORT/?platform=$2#$3" "$WORK/$1-$6.png" >/dev/null
}

mkdir -p "$OUT"
for scheme in light dark; do
  shoot desktop macos history "Desktop Chrome HiDPI" 1000,640 "$scheme"
  shoot phone android devices "Pixel 7" 412,860 "$scheme"
  border=$([ "$scheme" = dark ] && echo '#3f3f46' || echo '#d4d4d4')
  magick \( "$WORK/desktop-$scheme.png" -bordercolor "$border" -border 2 \) \
    \( "$WORK/phone-$scheme.png" -resize x1284 -bordercolor "$border" -border 2 \) \
    -background none -gravity center +smush 48 -resize 1600x "$WORK/combined-$scheme.png"
  cwebp -quiet -lossless -z 9 "$WORK/combined-$scheme.png" -o "$OUT/nearclip-$scheme.webp"
done
echo "Wrote $OUT/nearclip-light.webp and $OUT/nearclip-dark.webp"
