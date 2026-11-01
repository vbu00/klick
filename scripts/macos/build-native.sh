#!/bin/bash
# kl!ck β для Mac — родное окно на SwiftUI (crates/klick-mac): xcodegen → xcodebuild → dist/klick-mac-beta.zip.
# Подпись ad-hoc (бета не из App Store): первый запуск — правой кнопкой → «Открыть».
#   scripts/macos/build-native.sh [--shots]   --shots — ещё и снимки окна на тестовых данных (dist/mac-beta-*.png)
set -euo pipefail
[[ "$(uname -s)" == "Darwin" ]] || { echo "Собирать kl!ck β для Mac нужно на Mac с Xcode 26." >&2; exit 1; }
root="$(cd "$(dirname "$0")/../.." && pwd)"
mac="$root/crates/klick-mac"
dist="$root/dist"
shots=0
[[ "${1:-}" == "--shots" ]] && shots=1
command -v xcodegen >/dev/null || { echo "нет xcodegen: brew install xcodegen" >&2; exit 1; }
export KLICK_VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"
export KLICK_BUILD="${KLICK_BUILD:-1}"

echo "== проект"
(cd "$mac" && xcodegen generate --quiet)

echo "== сборка (arm64 + x86_64)"
xcodebuild -project "$mac/klick-mac.xcodeproj" -scheme KlickMac -configuration Release \
    -derivedDataPath "$mac/build" -destination 'generic/platform=macOS' \
    ARCHS="arm64 x86_64" ONLY_ACTIVE_ARCH=NO -quiet build
app="$mac/build/Build/Products/Release/kl!ck β.app"
[[ -d "$app" ]] || { echo "нет $app" >&2; exit 1; }
codesign --force --deep --sign - "$app"
mkdir -p "$dist"
rm -f "$dist/klick-mac-beta.zip"
ditto -c -k --keepParent "$app" "$dist/klick-mac-beta.zip"
echo "готово: $dist/klick-mac-beta.zip"

[[ $shots == 1 ]] || exit 0
echo "== снимки на тестовых данных"
bin="$app/Contents/MacOS/kl!ck β"
shot() { # shot <имя> <переменные…>
    local name="$1"; shift
    env KLICK_MOCK=1 "$@" "$bin" &
    local pid=$!
    sleep 6
    screencapture -x "$dist/mac-beta-$name.png" || true
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
    sleep 1
}
shot home-light KLICK_MOCK_THEME=light
shot home-dark KLICK_MOCK_THEME=dark
shot home-off-light KLICK_MOCK_THEME=light KLICK_MOCK_STATE=off
shot routes-light KLICK_MOCK_THEME=light KLICK_TAB=routes
shot settings-light KLICK_MOCK_THEME=light KLICK_TAB=settings
shot routes-dark KLICK_MOCK_THEME=dark KLICK_TAB=routes
ls -la "$dist"/mac-beta-*.png
