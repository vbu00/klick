#!/usr/bin/env bash
# Скриншоты для README: стенд вёрстки (node .claude/preview-server.js) +
# безголовый Edge. Окно 380×720 посреди страницы, плотность 2x.
#   bash tools/screenshots.sh
set -e
cd "$(dirname "$0")/.."
EDGE="/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"
OUT="$(cygpath -w "$PWD/docs/screenshots/")"
PROFILE="$(cygpath -w "$TEMP")\klick-shot"
BASE="http://localhost:4176/src/mock?frame=1"
shot() {
  "$EDGE" --headless=new --disable-gpu --hide-scrollbars --no-first-run --user-data-dir="$PROFILE" \
    --window-size=520,840 --force-device-scale-factor=2 --virtual-time-budget=40000 \
    --screenshot="$OUT$1.png" "$BASE&$2" 2>&1 | tail -1
}
shot home-dark        "theme=dark&state=on&expand=1&ping=1"
shot home-light       "theme=light&state=on&expand=1&ping=1"
shot home-midnight    "theme=custom:midnight:0a84ff&state=on"
shot rules-oled       "theme=custom:oled:bf5af2&screen=rules"
shot killswitch-light "theme=light&screen=kill&ks=sites"
shot theme-graphite   "theme=custom:graphite:ff9f0a&screen=theme"
shot mode-dark        "theme=dark&screen=mode"
shot add-light        "theme=light&screen=add"
shot about-dark       "theme=dark&screen=about"
shot ksinfo-dark      "theme=dark&screen=kill&info=ksInfo"
