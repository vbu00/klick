#!/usr/bin/env bash
# Картинки установщика NSIS из tools/installer-art.html: безголовый Edge
# снимает в 2x, Pillow режет и сохраняет BMP (NSIS понимает только его).
#   bash tools/make-installer-art.sh
set -e
cd "$(dirname "$0")/.."
EDGE="/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"
PAGE="file:///$(cygpath -m "$PWD/tools/installer-art.html")"
TMP="$(cygpath -w "$TEMP")"
mkdir -p src-tauri/installer
for kind in sidebar header; do
  "$EDGE" --headless=new --disable-gpu --hide-scrollbars --no-first-run --user-data-dir="$TMP\klick-art" \
    --window-size=600,700 --force-device-scale-factor=2 --virtual-time-budget=3000 \
    --screenshot="$TMP\klick-$kind.png" "$PAGE?kind=$kind" >/dev/null 2>&1
done
python - "$(cygpath -m "$TEMP")" <<'PY'
import sys
from PIL import Image
tmp = sys.argv[1]
for kind, (w, h) in {"sidebar": (164, 314), "header": (150, 57)}.items():
    im = Image.open(f"{tmp}/klick-{kind}.png").convert("RGB").crop((0, 0, w * 2, h * 2))
    im.save(f"src-tauri/installer/{kind}.bmp")
print("готово: src-tauri/installer/{sidebar,header}.bmp")
PY
