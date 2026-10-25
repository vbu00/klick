#!/bin/bash
# kl!ck β — родной интерфейс iOS (SwiftUI, crates/klick-ios/native): движок crates/klick-native (Rust),
# ядро mihomo (Go), расширение VPN — общие с веб-версией. Итог — dist/klick-ios-beta.ipa без подписи
# (подписывает тот, кто ставит, как и klick-ios.ipa), с --sim ещё и dist/klick-ios-beta-sim.app.
#   scripts/ios/build-native.sh [--sim]
set -euo pipefail

[[ "$(uname -s)" == "Darwin" ]] || { echo "Собирать kl!ck для iOS нужно на Mac с Xcode." >&2; exit 1; }

root="$(cd "$(dirname "$0")/../.." && pwd)"
ios="$root/crates/klick-ios"
native="$ios/native"
rust="$root/crates/klick-native"
dist="$root/dist"
sim=0
for a in "$@"; do
    case "$a" in
        --sim) sim=1 ;;
        *) echo "неизвестный аргумент: $a" >&2; exit 2 ;;
    esac
done

step() { echo; echo "== $*"; }

for cmd in cargo go xcodebuild codesign xcodegen; do
    command -v "$cmd" >/dev/null || { echo "нет $cmd: нужны Xcode, Go, Rust и xcodegen (brew install xcodegen)" >&2; exit 1; }
done
targets=(aarch64-apple-ios)
[[ $sim == 1 ]] && targets+=(aarch64-apple-ios-sim)
if command -v rustup >/dev/null; then rustup target add "${targets[@]}" >/dev/null; fi

version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"
own="$(sed -n 's/^version = "\(.*\)"/\1/p' "$rust/Cargo.toml" | head -1)"
[[ "$version" == "$own" ]] || { echo "версия crates/klick-native ($own) не совпадает с версией kl!ck ($version) в Cargo.toml" >&2; exit 1; }
export KLICK_VERSION="$version" KLICK_BUILD="${KLICK_BUILD:-1}"
export KLICK_RELEASES_REPO="${KLICK_RELEASES_REPO:-}"
export IPHONEOS_DEPLOYMENT_TARGET=15.0

step "наборы, каталог сервисов и база стран"
"$root/scripts/macos/fetch-core.sh"
res="$native/klick-resources"
rm -rf "$res"
mkdir -p "$res/core"
cp -R "$root/resources/sets" "$res/"
cp "$root/resources/catalog.json" "$res/"
cp "$root/resources/core/Country.mmdb" "$res/core/"

step "значки веб-версии"
# Те же SVG, что у веб-версии (ui/src/assets): значки — шаблоном (цвет — цвет текста), значки
# состояния — как есть. SVG без размеров Xcode не рисует — размер подставляется.
icons="$native/Icons.xcassets"
rm -rf "$icons"
mkdir -p "$icons"
echo '{"info":{"author":"xcode","version":1}}' >"$icons/Contents.json"
add_icon() { # add_icon <svg> <имя> <template|original>
    local set="$icons/$2.imageset"
    mkdir -p "$set"
    perl -pe 's/<svg (?![^>]*width=)/<svg width="64" height="64" /' "$1" >"$set/$2.svg"
    cat >"$set/Contents.json" <<JSON
{"images":[{"filename":"$2.svg","idiom":"universal"}],"info":{"author":"xcode","version":1},
 "properties":{"preserves-vector-representation":true,"template-rendering-intent":"$3"}}
JSON
}
for f in "$root"/ui/src/assets/icons/*.svg; do add_icon "$f" "$(basename "$f" .svg)" template; done
for f in "$root"/ui/src/assets/status/*.svg; do add_icon "$f" "status-$(basename "$f" .svg)" original; done
echo "$(ls "$icons" | grep -c imageset) шт."

step "ядро mihomo для iOS (Go)"
if [[ -f "$ios/core/build/iphoneos/libklickcore.a" && -f "$ios/core/build/iphonesimulator/libklickcore.a" ]]; then
    echo "уже собрано (scripts/ios/build.sh)"
else
    "$ios/core/build.sh"
fi

step "движок kl!ck (Rust)"
for t in "${targets[@]}"; do
    cargo build --release --locked --manifest-path "$rust/Cargo.toml" --target "$t"
    platform=iphoneos
    [[ "$t" == *-sim ]] && platform=iphonesimulator
    mkdir -p "$native/lib/$platform"
    cp "$rust/target/$t/release/libklick_native.a" "$native/lib/$platform/"
done

step "проект Xcode"
(cd "$native" && xcodegen generate --spec project.yml)
rm -rf "$native/build"

step "приложение и расширение VPN (устройство)"
xcodebuild -project "$native/klick-native.xcodeproj" -scheme KlickNative -configuration Release \
    -sdk iphoneos -destination 'generic/platform=iOS' -archivePath "$native/build/KlickNative.xcarchive" \
    CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO archive -quiet
app="$native/build/KlickNative.xcarchive/Products/Applications/KlickNative.app"
[[ -d "$app" ]] || { echo "архив Xcode без приложения" >&2; exit 1; }

step "подпись без сертификата с правами (ad-hoc)"
codesign --force --sign - --timestamp=none --entitlements "$native/KlickNativeTunnel.entitlements" "$app/PlugIns/KlickTunnel.appex"
codesign --force --sign - --timestamp=none --entitlements "$native/KlickNative.entitlements" "$app"

step "klick-ios-beta.ipa"
mkdir -p "$dist"
rm -f "$dist/klick-ios-beta.ipa"
payload="$(mktemp -d)"
trap 'rm -rf "$payload"' EXIT
mkdir "$payload/Payload"
cp -R "$app" "$payload/Payload/"
(cd "$payload" && zip -qry "$dist/klick-ios-beta.ipa" Payload)
ls -la "$dist/klick-ios-beta.ipa"

if [[ $sim == 1 ]]; then
    step "kl!ck β для симулятора"
    rm -rf "$dist/klick-ios-beta-sim.app"
    xcodebuild -project "$native/klick-native.xcodeproj" -scheme KlickNative -configuration Release \
        -sdk iphonesimulator -destination 'generic/platform=iOS Simulator' -derivedDataPath "$native/build/sim" \
        CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO build -quiet
    built="$native/build/sim/Build/Products/Release-iphonesimulator/KlickNative.app"
    [[ -d "$built" ]] || { echo "сборка для симулятора без приложения" >&2; exit 1; }
    cp -R "$built" "$dist/klick-ios-beta-sim.app"
    echo "dist/klick-ios-beta-sim.app"
fi
