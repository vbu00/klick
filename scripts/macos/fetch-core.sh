#!/bin/bash
# Скачивает ядро mihomo для macOS (arm64 и x86_64), сверяет SHA-256 и кладёт универсальный файл
# в resources/core/mihomo; рядом — базу стран Country.mmdb. Нужно для сборки и для службы в режиме
# разработки. Версии и файлы — те же, что у Windows-сборки (tools/fetch-core.ps1).
#
#   scripts/macos/fetch-core.sh           # универсальный (lipo), на Linux — под текущую архитектуру
#   scripts/macos/fetch-core.sh --force   # скачать заново
set -euo pipefail

VERSION="v1.19.33"
# SHA-256 архивов .gz с https://github.com/MetaCubeX/mihomo/releases/tag/v1.19.33
SHA_ARM64="060b64b0f2c9a310831db0fe4adb2878de67bfa606c311813c1f13966aa504b6"
SHA_AMD64="551ae54ef8d77a5e21f7a8185a59b441fb13a0fb0514fcdef5ca2816f6f50015"
SHA_LINUX_AMD64="132f6f3edf9a65e62f04853aed3358472e0b2b7666daa8419edfa2ddf5bb6323"
# База стран из MetaCubeX/meta-rules-dat, привязана к коммиту.
GEO_COMMIT="6b01e65c89cdeb684c00c70bb0b414091b22f124"
GEO_SHA="21606dfffd4ec39542ec40782bebd37b71310471816e1338b6f6430190cbc85d"

root="$(cd "$(dirname "$0")/../.." && pwd)"
out="$root/resources/core/mihomo"
mmdb="$root/resources/core/Country.mmdb"
force=0
[[ "${1:-}" == "--force" ]] && force=1
if [[ -x "$out" && -f "$mmdb" && $force == 0 ]]; then
    echo "уже есть: $out ($("$out" -v 2>/dev/null | head -1 || echo '?')) и Country.mmdb"
    exit 0
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

sha256() { if command -v shasum >/dev/null; then shasum -a 256 "$1" | cut -d' ' -f1; else sha256sum "$1" | cut -d' ' -f1; fi; }

check_sha() { # check_sha <файл> <sha256>
    local got; got="$(sha256 "$1")"
    if [[ "$got" != "$2" ]]; then
        echo "SHA-256 не совпал для $(basename "$1"): $got (ждали $2)" >&2
        exit 1
    fi
}

fetch() { # fetch <platform> <sha256>
    local name="mihomo-$1-$VERSION.gz"
    echo "== $name"
    curl -fL --retry 3 -o "$tmp/$name" "https://github.com/MetaCubeX/mihomo/releases/download/$VERSION/$name"
    check_sha "$tmp/$name" "$2"
    gunzip -c "$tmp/$name" > "$tmp/mihomo-$1"
    chmod +x "$tmp/mihomo-$1"
}

mkdir -p "$(dirname "$out")"
if [[ ! -f "$mmdb" || $force == 1 ]]; then
    echo "== Country.mmdb"
    curl -fL --retry 3 -o "$tmp/Country.mmdb" "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/$GEO_COMMIT/country.mmdb"
    check_sha "$tmp/Country.mmdb" "$GEO_SHA"
    mv "$tmp/Country.mmdb" "$mmdb"
fi
if [[ -x "$out" && $force == 0 ]]; then
    echo "готово: $out, $mmdb"
    exit 0
fi

if [[ "$(uname -s)" == "Darwin" ]]; then
    fetch darwin-arm64 "$SHA_ARM64"
    fetch darwin-amd64 "$SHA_AMD64"
    lipo -create -output "$tmp/mihomo" "$tmp/mihomo-darwin-arm64" "$tmp/mihomo-darwin-amd64"
    # Файлы из интернета macOS помечает карантином — снять, иначе служба может не запустить ядро.
    xattr -c "$tmp/mihomo" 2>/dev/null || true
else
    # Linux: только чтобы гонять службу для разработки и проверять конфиги (mihomo -t).
    fetch linux-amd64 "$SHA_LINUX_AMD64"
    mv "$tmp/mihomo-linux-amd64" "$tmp/mihomo"
fi

mv "$tmp/mihomo" "$out"
echo "готово: $out ($("$out" -v | head -1))"
