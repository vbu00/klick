#!/bin/bash
# kl!ck для Android: APK в dist/ — общий klick-android.apk и по архитектурам (arm64, armv7, x86_64 для эмулятора).
#   1. ядро mihomo — Go библиотекой (`-buildmode=c-shared`, crates/klick-ios/core) для каждой архитектуры;
#   2. движок — Rust (crates/klick-android, cargo-ndk);
#   3. наборы, каталог сервисов и база стран — в assets;
#   4. Gradle (crates/klick-android) — приложение.
# Нужны: Go, Rust с целями Android, cargo-ndk, Android SDK и NDK (ANDROID_NDK_HOME), JDK 17, Gradle.
#   scripts/android/build.sh            # все архитектуры
#   ABIS="x86_64" scripts/android/build.sh   # только эмулятор
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
app="$root/crates/klick-android"
step() { echo; echo "== $*"; }

: "${ANDROID_NDK_HOME:?нужен ANDROID_NDK_HOME — папка NDK}"
ABIS="${ABIS:-arm64-v8a armeabi-v7a x86_64}"
# Как minSdk приложения (app/build.gradle.kts).
API=26

version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"
own="$(sed -n 's/^version = "\(.*\)"/\1/p' "$app/Cargo.toml" | head -1)"
[[ "$version" == "$own" ]] || { echo "версия crates/klick-android ($own) не та, что у kl!ck ($version)" >&2; exit 1; }

host="$(uname -s | tr '[:upper:]' '[:lower:]')-x86_64"
toolchain="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$host/bin"
jni="$app/app/src/main/jniLibs"

step "ядро mihomo (Go)"
core="$root/crates/klick-ios/core"
mihomo="$(cd "$core" && go list -m -f '{{.Version}}' github.com/metacubex/mihomo)"
# Те же теги, что у iOS, но без with_low_memory (у службы Android нет предела в 50 МБ) и с cmfa — как у
# ClashMetaForAndroid: без него адаптер читает список приложений системы, а приложению это запрещено
# (подробнее — jni_android.go).
tags="with_gvisor,no_tailscale,no_zerotier,no_easytier,no_fake_tcp,cmfa"
for abi in $ABIS; do
    case "$abi" in
        arm64-v8a) goarch=arm64; cc="aarch64-linux-android$API-clang"; extra="" ;;
        armeabi-v7a) goarch=arm; cc="armv7a-linux-androideabi$API-clang"; extra="GOARM=7" ;;
        x86_64) goarch=amd64; cc="x86_64-linux-android$API-clang"; extra="" ;;
        *) echo "не знаю архитектуру $abi" >&2; exit 1 ;;
    esac
    mkdir -p "$jni/$abi"
    # 16 КБ страницы: Android 15+ на новых телефонах требует выравнивания библиотек.
    (cd "$core" && env $extra CGO_ENABLED=1 GOOS=android GOARCH="$goarch" CC="$toolchain/$cc" \
        CGO_LDFLAGS="-Wl,-z,max-page-size=16384" \
        go build -buildmode=c-shared -tags "$tags" -trimpath \
        -ldflags "-s -w -X github.com/metacubex/mihomo/constant.Version=$mihomo" \
        -o "$jni/$abi/libklickcore.so" .)
    rm -f "$jni/$abi/libklickcore.h"
done

step "движок (Rust)"
targets=()
for abi in $ABIS; do targets+=(-t "$abi"); done
(cd "$app" && cargo ndk --platform "$API" "${targets[@]}" -o "$jni" build --release)

step "наборы, каталог сервисов и база стран"
"$root/scripts/macos/fetch-core.sh"
res="$app/app/src/main/assets/klick-resources"
rm -rf "$res"
mkdir -p "$res/core"
cp -R "$root/resources/sets" "$res/"
cp "$root/resources/catalog.json" "$res/"
cp "$root/resources/core/Country.mmdb" "$res/core/"

step "приложение (Gradle)"
(cd "$app" && gradle --no-daemon -q assembleRelease)
mkdir -p "$root/dist"
out="$app/app/build/outputs/apk/release"
# Общий — klick-android.apk; под архитектуру — klick-android-arm64.apk и т. д. (почти все телефоны — arm64).
cp "$out/app-universal-release.apk" "$root/dist/klick-android.apk"
for pair in arm64-v8a:arm64 armeabi-v7a:armv7 x86_64:x86_64; do
    abi="${pair%%:*}"
    if [[ -f "$out/app-$abi-release.apk" ]]; then cp "$out/app-$abi-release.apk" "$root/dist/klick-android-${pair##*:}.apk"; fi
done
ls -la "$root"/dist/klick-android*.apk
