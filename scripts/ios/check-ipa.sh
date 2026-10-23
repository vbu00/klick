#!/bin/bash
# Проверяет klick-ios.ipa: приложение и расширение VPN на месте, права в подписи, arm64, iOS 15+,
# iPhone и iPad, версия как у kl!ck, ядро mihomo вошло и в приложение, и в расширение.
#   scripts/ios/check-ipa.sh dist/klick-ios.ipa
set -euo pipefail
ipa="${1:?путь к .ipa}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
unzip -q "$ipa" -d "$tmp"
app="$(find "$tmp/Payload" -maxdepth 1 -name '*.app' -type d | head -1)"
appex="$app/PlugIns/KlickTunnel.appex"
bad=0
check() { # check <описание> <команда…>
    local what="$1"; shift
    if "$@" >/dev/null 2>&1; then echo "ok   $what"; else echo "FAIL $what"; bad=1; fi
}
plist() { /usr/libexec/PlistBuddy -c "Print :$2" "$1/Info.plist" 2>/dev/null; }
has() { [[ "$1" == *"$2"* ]]; }
ents() { ldid -e "$1" 2>/dev/null; }

check "приложение klick.app" test -x "$app/klick"
check "расширение VPN KlickTunnel.appex" test -x "$appex/KlickTunnel"
check "расширение — Packet Tunnel" has "$(plist "$appex" NSExtension:NSExtensionPointIdentifier)" com.apple.networkextension.packet-tunnel
check "права приложения: VPN" has "$(ents "$app/klick")" packet-tunnel-provider
check "права приложения: App Group" has "$(ents "$app/klick")" com.apple.security.application-groups
check "права расширения: VPN" has "$(ents "$appex/KlickTunnel")" packet-tunnel-provider
check "права расширения: App Group" has "$(ents "$appex/KlickTunnel")" com.apple.security.application-groups
check "arm64 у приложения" has "$(lipo -archs "$app/klick")" arm64
check "arm64 у расширения" has "$(lipo -archs "$appex/KlickTunnel")" arm64
check "iOS 15+" test "$(plist "$app" MinimumOSVersion)" = 15.0
check "iPhone и iPad" has "$(plist "$app" UIDeviceFamily)" 2
check "версия $version" test "$(plist "$app" CFBundleShortVersionString)" = "$version"
check "версия расширения $version" test "$(plist "$appex" CFBundleShortVersionString)" = "$version"
check "id расширения внутри id приложения" has "$(plist "$appex" CFBundleIdentifier)" "$(plist "$app" CFBundleIdentifier)."
check "ссылки klick://" has "$(plist "$app" CFBundleURLTypes)" klick
check "ядро mihomo в приложении" has "$(strings -a "$app/klick" | grep -m1 'metacubex/mihomo')" mihomo
check "ядро mihomo в расширении" has "$(strings -a "$appex/KlickTunnel" | grep -m1 'metacubex/mihomo')" mihomo
check "наборы и база стран" test -f "$app/klick-resources/core/Country.mmdb" -a -f "$app/klick-resources/catalog.json"
check "иконка" test -n "$(find "$app" -maxdepth 1 -name 'AppIcon*.png' -o -maxdepth 1 -name 'Assets.car' | head -1)"
echo
ls -la "$ipa"
du -sh "$app/klick" "$appex/KlickTunnel" | sed 's|'"$tmp"'/||'
exit $bad
