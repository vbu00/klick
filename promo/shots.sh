#!/usr/bin/env bash
# Кадры приложения для ролика: стенд вёрстки (node .claude/preview-server.js)
# + безголовый Edge в 2x. Окно 380×720 вырезается из кадра 520×840.
#   bash promo/shots.sh
set -e
cd "$(dirname "$0")/.."
EDGE="/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"
TMP="$(cygpath -w "$TEMP")"
shot() {
  "$EDGE" --headless=new --disable-gpu --hide-scrollbars --no-first-run --user-data-dir="$TMP\klick-promo" \
    --window-size=520,840 --force-device-scale-factor=2 --virtual-time-budget="${3:-8000}" \
    --screenshot="$TMP\promo-$1.png" "http://localhost:4176/src/mock?frame=1&$2" >/dev/null 2>&1
  python -c "from PIL import Image; Image.open(r'$TMP\promo-$1.png').convert('RGB').crop((140,120,900,1560)).save('promo/assets/$1.png', optimize=True)"
  echo "$1"
}
shot off   "theme=dark"
shot on    "theme=dark&state=on&ping=1" 40000
"$EDGE" --headless=new --disable-gpu --hide-scrollbars --no-first-run --user-data-dir="$TMP\klick-promo" \
  --window-size=380,700 --force-device-scale-factor=2 --virtual-time-budget=6000 \
  --screenshot="$TMP\promo-tray.png" "http://localhost:4176/src/tray?state=on&theme=dark&bg=%23000000" >/dev/null 2>&1
python -c "
from PIL import Image
im=Image.open(r'$TMP\promo-tray.png').convert('RGB').crop((0,0,720,1150))
im.save('promo/assets/tray.png', optimize=True)"
echo tray
