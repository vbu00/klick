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
# Геймерский набор (?promo=1): выбор CS2 из запущенных и правила после.
shot picker "theme=dark&promo=1&state=on&screen=rules&rtab=apps&picker=1&pick=cs2.exe" 40000
shot rules  "theme=dark&promo=1&apps=after&screen=rules&rtab=apps&scroll=end"
# Меню трея: карточка 360 px в окне 380 — снимаем и режем верх.
tray() {
  "$EDGE" --headless=new --disable-gpu --hide-scrollbars --no-first-run --user-data-dir="$TMP\klick-promo" \
    --window-size=380,700 --force-device-scale-factor=2 --virtual-time-budget=6000 \
    --screenshot="$TMP\promo-$1.png" "http://localhost:4176/src/tray?state=on&theme=dark&bg=%23000000&$2" >/dev/null 2>&1
  python -c "from PIL import Image; Image.open(r'$TMP\promo-$1.png').convert('RGB').crop((0,0,720,1150)).save('promo/assets/$1.png', optimize=True)"
  echo "$1"
}
# Для ролика: сейчас Frankfurt (96 мс) → переключились на Amsterdam (31 мс).
tray tray-before "promo=1"
tray tray-after  "promo=1&active=Amsterdam"
# Пролёт по приложению (flythrough): режим «Системный proxy», без «VPN» в кадре.
shot fly-off   "theme=dark&mode=sysproxy"
shot fly-on    "theme=dark&mode=sysproxy&state=on&expand=1&ping=1" 40000
shot fly-add   "theme=dark&screen=add&link=https%3A%2F%2Fpanel.example.com%2Fsub%2Fa1b2c3d4"
shot fly-rules "theme=dark&screen=rules"
shot fly-ks    "theme=dark&screen=kill&ks=sites"
shot fly-theme "theme=custom:midnight:0a84ff&screen=theme"
tray fly-tray  ""
shot fly-servers "theme=dark&mode=sysproxy&state=on&expand=1&ping=1&scroll=250" 40000
