#!/bin/bash
# Собирает kl!ck для macOS: страницы окна, служба, утилита, ядро → dist/kl!ck.app и dist/klick-<версия>.pkg.
#
#   scripts/macos/build.sh             # Apple Silicon + Intel в одном файле (universal), release
#   scripts/macos/build.sh --native    # только архитектура этого Mac — быстрее, для проверки
#   scripts/macos/build.sh --debug     # отладочная сборка
#   scripts/macos/build.sh --no-pkg    # только kl!ck.app
#
# Подпись и нотаризация (без них сборка подписывается «ad-hoc» и открывается только правым щелчком → «Открыть»):
#   KLICK_SIGN_IDENTITY="Developer ID Application: Имя (TEAMID)"   подпись приложения, службы и ядра
#   KLICK_INSTALLER_IDENTITY="Developer ID Installer: Имя (TEAMID)" подпись .pkg
#   KLICK_NOTARY_PROFILE=<профиль из `xcrun notarytool store-credentials`>  нотаризация .pkg
set -euo pipefail

[[ "$(uname -s)" == "Darwin" ]] || { echo "Собирать kl!ck для macOS нужно на Mac (Xcode Command Line Tools)." >&2; exit 1; }

root="$(cd "$(dirname "$0")/../.." && pwd)"
native=0; debug=0; pkg=1
for a in "$@"; do
    case "$a" in
        --native) native=1 ;;
        --debug) debug=1 ;;
        --no-pkg) pkg=0 ;;
        *) echo "неизвестный аргумент: $a" >&2; exit 2 ;;
    esac
done

# Ядро mihomo собрано Go 1.26, а он работает на macOS 12 Monterey и новее.
export MACOSX_DEPLOYMENT_TARGET=12.0
profile=$([[ $debug == 1 ]] && echo debug || echo release)
cargo_flag=$([[ $debug == 1 ]] && echo "" || echo "--release")
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"
host="$(rustc -vV | sed -n 's/^host: //p')"
if [[ $native == 1 ]]; then
    targets=("$host"); triple="$host"
    arch=$([[ "$host" == aarch64-* ]] && echo arm64 || echo x86_64)
else
    targets=(aarch64-apple-darwin x86_64-apple-darwin); triple="universal-apple-darwin"
    arch="arm64,x86_64"
fi

step() { echo; echo "== $*"; }

for cmd in cargo npm lipo codesign pkgbuild productbuild; do
    command -v "$cmd" >/dev/null || { echo "нет $cmd: нужны Rust, Node.js 20.19+ и Xcode Command Line Tools (xcode-select --install)" >&2; exit 1; }
done
if command -v rustup >/dev/null; then rustup target add "${targets[@]}" >/dev/null; fi

step "ядро mihomo"
"$root/scripts/macos/fetch-core.sh"

step "страницы окна"
(cd "$root/ui" && { [[ -d node_modules ]] || npm ci --no-audit --no-fund; } && npm run build)

step "служба и утилита (${targets[*]})"
for t in "${targets[@]}"; do
    (cd "$root" && cargo build $cargo_flag --target "$t" -p klick-service -p klick-cli)
done

step "файлы для пакета .app"
# Tauri ищет их по имени с целевой платформой: при сборке окна под каждую архитектуру
# (`…-aarch64-apple-darwin`, `…-x86_64-apple-darwin`) и при упаковке (`…-universal-apple-darwin`).
# Универсальный файл годится для всех трёх.
bin="$root/crates/klick-ui/binaries"
rm -rf "${bin:?}" && mkdir -p "$bin"
names=("$triple")
[[ $native == 1 ]] || names+=("${targets[@]}")
for exe in klick-service klick-cli mihomo; do
    if [[ $exe == mihomo ]]; then
        cp "$root/resources/core/mihomo" "$bin/$exe"
    else
        parts=()
        for t in "${targets[@]}"; do parts+=("$root/target/$t/$profile/$exe"); done
        lipo -create -output "$bin/$exe" "${parts[@]}"
    fi
    for n in "${names[@]}"; do cp "$bin/$exe" "$bin/$exe-$n"; done
    rm "$bin/$exe"
done
xattr -c "$bin"/* 2>/dev/null || true

step "kl!ck.app"
export APPLE_SIGNING_IDENTITY="${KLICK_SIGN_IDENTITY:--}"
tauri="$root/ui/node_modules/.bin/tauri"
tauri_flags=(build --target "$triple" --bundles app --config "$root/scripts/macos/bundle.json")
if [[ $debug == 1 ]]; then tauri_flags+=(--debug); fi
(cd "$root/crates/klick-ui" && "$tauri" "${tauri_flags[@]}")
app="$root/target/$triple/$profile/bundle/macos/kl!ck.app"
[[ -d "$app" ]] || { echo "не нашёл $app" >&2; exit 1; }
if ! codesign --verify --deep --strict "$app" 2>/dev/null; then
    echo "подпись не прошла проверку — подписываю ad-hoc"
    codesign --force --deep --sign - "$app"
fi

mkdir -p "$root/dist"
rm -rf "$root/dist/kl!ck.app"
ditto "$app" "$root/dist/kl!ck.app"
echo "готово: dist/kl!ck.app"

if [[ $pkg == 1 ]]; then
    step "установщик .pkg"
    "$root/scripts/macos/make-pkg.sh" "$root/dist/kl!ck.app" "$version" "$arch" "$root/dist/klick-$version.pkg"
    if [[ -n "${KLICK_NOTARY_PROFILE:-}" ]]; then
        step "нотаризация"
        xcrun notarytool submit "$root/dist/klick-$version.pkg" --keychain-profile "$KLICK_NOTARY_PROFILE" --wait
        xcrun stapler staple "$root/dist/klick-$version.pkg"
    fi
    echo "готово: dist/klick-$version.pkg"
fi
