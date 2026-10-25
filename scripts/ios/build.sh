#!/bin/bash
# Собирает kl!ck для iOS (iPhone и iPad, arm64, iOS 15+) → dist/klick-ios.ipa.
#
#   scripts/ios/build.sh          # .ipa для устройства
#   scripts/ios/build.sh --sim    # ещё и kl!ck для симулятора (Mac с Apple Silicon) → dist/klick-ios-sim.app
#
# .ipa не подписан сертификатом: подписывает его тот, кто ставит (Sideloadly, SideStore, ESign, Feather…,
# см. docs/ios-install.md). Права — расширение VPN (packet-tunnel-provider) и общая папка App Group —
# вписаны в подпись без сертификата (ad-hoc): по ним подписчики понимают, какие возможности включить.
#
# Нужны Mac с Xcode, Go, Rust (цели aarch64-apple-ios и aarch64-apple-ios-sim), Node.js и xcodegen
# (`brew install xcodegen`).
set -euo pipefail

[[ "$(uname -s)" == "Darwin" ]] || { echo "Собирать kl!ck для iOS нужно на Mac с Xcode." >&2; exit 1; }

root="$(cd "$(dirname "$0")/../.." && pwd)"
ios="$root/crates/klick-ios"
apple="$ios/gen/apple"
dist="$root/dist"
sim=0
for a in "$@"; do
    case "$a" in
        --sim) sim=1 ;;
        *) echo "неизвестный аргумент: $a" >&2; exit 2 ;;
    esac
done

step() { echo; echo "== $*"; }

for cmd in cargo go npm xcodebuild codesign xcodegen; do
    command -v "$cmd" >/dev/null || { echo "нет $cmd: нужны Xcode, Go, Rust, Node.js и xcodegen (brew install xcodegen)" >&2; exit 1; }
done
if command -v rustup >/dev/null; then rustup target add aarch64-apple-ios aarch64-apple-ios-sim >/dev/null; fi

version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"
own="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ios/Cargo.toml" | head -1)"
[[ "$version" == "$own" ]] || { echo "версия crates/klick-ios ($own) не совпадает с версией kl!ck ($version) в Cargo.toml" >&2; exit 1; }
export KLICK_VERSION="$version" KLICK_BUILD="${KLICK_BUILD:-1}"
# Проверка обновлений в окне смотрит выпуски этого репозитория.
export KLICK_RELEASES_REPO="${KLICK_RELEASES_REPO:-}"

step "страницы окна"
(cd "$root/ui" && { [[ -d node_modules ]] || npm ci --no-audit --no-fund; } && npm run build)

step "наборы, каталог сервисов и база стран"
"$root/scripts/macos/fetch-core.sh"
res="$apple/klick-resources"
rm -rf "$res"
mkdir -p "$res/core"
cp -R "$root/resources/sets" "$res/"
cp "$root/resources/catalog.json" "$res/"
cp "$root/resources/core/Country.mmdb" "$res/core/"

step "ядро mihomo для iOS (Go)"
"$ios/core/build.sh"

step "проект Xcode"
mkdir -p "$apple"/Externals/{arm64,x86_64}/{debug,release}
(cd "$apple" && xcodegen generate --spec project.yml)

tauri="$root/ui/node_modules/.bin/tauri"
# Версия приложения — та же, что у kl!ck (иначе Tauri ставит 1.0.0).
config="{\"version\":\"$version\"}"
rm -rf "$apple/build"

step "приложение и расширение VPN (устройство)"
(cd "$ios" && "$tauri" ios build --target aarch64 --no-sign --archive-only --ci --config "$config")
app="$(find "$apple/build" -maxdepth 5 -type d -path '*.xcarchive/Products/Applications/*.app' | head -1)"
[[ -d "$app" ]] || { echo "архив Xcode без приложения" >&2; exit 1; }

step "подпись без сертификата с правами (ad-hoc)"
# Изнутри наружу: библиотеки, расширение, приложение.
if [[ -d "$app/Frameworks" ]]; then
    find "$app/Frameworks" -maxdepth 1 \( -name '*.dylib' -o -name '*.framework' \) -exec codesign --force --sign - --timestamp=none {} \;
fi
codesign --force --sign - --timestamp=none --entitlements "$apple/KlickTunnel/KlickTunnel.entitlements" "$app/PlugIns/KlickTunnel.appex"
codesign --force --sign - --timestamp=none --entitlements "$apple/klick-ios_iOS/klick-ios_iOS.entitlements" "$app"

step "klick-ios.ipa"
mkdir -p "$dist"
rm -f "$dist/klick-ios.ipa"
payload="$(mktemp -d)"
trap 'rm -rf "$payload"' EXIT
mkdir "$payload/Payload"
cp -R "$app" "$payload/Payload/"
(cd "$payload" && zip -qry "$dist/klick-ios.ipa" Payload)
"$root/scripts/ios/check-ipa.sh" "$dist/klick-ios.ipa"

if [[ $sim == 1 ]]; then
    step "kl!ck для симулятора"
    rm -rf "$apple/build" "$dist/klick-ios-sim.app"
    (cd "$ios" && "$tauri" ios build --target aarch64-sim --no-sign --ci --config "$config")
    built="$(find "$apple/build" -name '*.app' -type d -not -path '*/PlugIns/*' | head -1)"
    [[ -d "$built" ]] || { echo "сборка для симулятора без приложения" >&2; exit 1; }
    cp -R "$built" "$dist/klick-ios-sim.app"
    echo "dist/klick-ios-sim.app"
fi
