#!/bin/bash
# Ядро mihomo для iOS статической библиотекой (`-buildmode=c-archive`): build/iphoneos — устройство
# (arm64), build/iphonesimulator — симулятор на Mac с Apple Silicon. Нужны Go и Xcode (только на Mac).
# Её линкуют и приложение (проверочное ядро), и расширение VPN (ядро туннеля), см. gen/apple/project.yml.
set -euo pipefail
cd "$(dirname "$0")"

version="$(go list -m -f '{{.Version}}' github.com/metacubex/mihomo)"
# Минимальная iOS — как у приложения (tauri.conf.json, project.yml): с iOS 15 расширению дают 50 МБ памяти.
min_ios=15.0
# Теги mihomo: стек gVisor; буферы соединений вдвое меньше (with_low_memory: 16 КБ на TCP, 8 КБ на UDP) —
# расширению дают 50 МБ на всё; без Tailscale, ZeroTier и EasyTier — редкие и тяжёлые виды серверов.
tags="with_gvisor,with_low_memory,no_tailscale,no_zerotier,no_easytier,no_fake_tcp"

build() { # build <sdk> <цель clang>
    local sdk="$1" target="$2" sysroot
    sysroot="$(xcrun --sdk "$sdk" --show-sdk-path)"
    mkdir -p "build/$sdk"
    CGO_ENABLED=1 GOOS=ios GOARCH=arm64 \
        CC="$(xcrun --sdk "$sdk" --find clang)" \
        CGO_CFLAGS="-isysroot $sysroot -target $target -O2" \
        CGO_LDFLAGS="-isysroot $sysroot -target $target" \
        go build -buildmode=c-archive -tags "$tags" -trimpath \
        -ldflags "-X github.com/metacubex/mihomo/constant.Version=$version" \
        -o "build/$sdk/libklickcore.a" .
}

build iphoneos "arm64-apple-ios$min_ios"
build iphonesimulator "arm64-apple-ios$min_ios-simulator"
ls -la build/*/libklickcore.a
