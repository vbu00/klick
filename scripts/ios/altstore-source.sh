#!/bin/bash
# Источник приложений для AltStore, SideStore, Feather и ESign: dist/klick-altstore.json для выпуска
# v<версия> в репозитории GITHUB_REPOSITORY. Один файл понимают все четыре: список версий `versions`
# (AltStore, SideStore, Feather) и те же сведения плоско в приложении (ESign и старые AltStore).
# Права в `appPermissions` должны совпадать с правами в .ipa — иначе AltStore и SideStore не поставят.
#   scripts/ios/altstore-source.sh dist/klick-ios.ipa
set -euo pipefail
ipa="${1:?путь к .ipa}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
repo="${GITHUB_REPOSITORY:-limeflash/klick}"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"
size="$(wc -c <"$ipa" | tr -d ' ')"
sha="$(shasum -a 256 "$ipa" | cut -d' ' -f1)"
date="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
url="https://github.com/$repo/releases/download/v$version/klick-ios.ipa"
icon="https://raw.githubusercontent.com/$repo/main/docs/logo/blue-dark.png"
about="VPN-клиент на ядре mihomo: Hysteria2, VLESS, AmneziaWG и другие из подписки, правила по сайтам и сервисам, «Заблокированное — через VPN». Нужна подпись с правами Network Extension и App Groups (платный аккаунт разработчика) — см. docs/ios-install.md."
notes="kl!ck $version для iPhone и iPad."

mkdir -p "$root/dist"
jq -n \
    --arg version "$version" --arg date "$date" --arg url "$url" --arg icon "$icon" --arg about "$about" \
    --arg notes "$notes" --arg sha "$sha" --arg repo "$repo" --argjson size "$size" '
{
  name: "kl!ck",
  identifier: "app.klick.source",
  subtitle: "VPN-клиент на mihomo",
  description: $about,
  iconURL: $icon,
  website: ("https://github.com/" + $repo),
  tintColor: "#007AFF",
  version: 2,
  apps: [{
    name: "kl!ck",
    bundleIdentifier: "app.klick.ios",
    developerName: "kl!ck",
    subtitle: "VPN-клиент на mihomo",
    localizedDescription: $about,
    iconURL: $icon,
    tintColor: "#007AFF",
    category: "utilities",
    screenshots: [],
    versions: [{
      version: $version,
      buildVersion: "1",
      date: $date,
      localizedDescription: $notes,
      downloadURL: $url,
      size: $size,
      sha256: $sha,
      minOSVersion: "15.0"
    }],
    version: $version,
    versionDate: $date,
    versionDescription: $notes,
    downloadURL: $url,
    size: $size,
    appPermissions: {
      entitlements: ["com.apple.developer.networking.networkextension", "com.apple.security.application-groups"],
      privacy: {}
    }
  }],
  news: []
}' >"$root/dist/klick-altstore.json"
echo "dist/klick-altstore.json → $url"
