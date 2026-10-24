#!/bin/bash
# Текст выпуска для Windows, macOS и iOS сразу: что нового (docs/release-notes.md) и как поставить.
# Выпуск один на версию — тег v<версия>, в нём установщик для Windows, пакет для Mac и .ipa; текст пишет
# тот, кто выкладывает первым (scripts/macos/publish-release.sh, scripts/ios/publish-release.sh или выпуск
# в .github/workflows/windows.yml).
#   scripts/release-notes.sh <версия> <владелец/репозиторий>
set -euo pipefail
cd "$(dirname "$0")/.."
ver="${1:?нужна версия}"
repo="${2:?нужен владелец/репозиторий}"
cat <<NOTES
kl!ck $ver для Windows 10/11 x64, macOS 12+ (Apple Silicon и Intel), iPhone и iPad (iOS 15+).

$(cat docs/release-notes.md)

**Windows:** скачать \`klick-$ver-x64-setup.exe\` ниже и запустить — поверх прежней kl!ck подключения и настройки остаются.

**macOS:** пакет не подписан сертификатом Apple, поэтому ставится из Терминала (так без вопроса Gatekeeper):

\`\`\`sh
curl -fL -o /tmp/klick.pkg https://github.com/$repo/releases/download/v$ver/klick-macos.pkg
sudo installer -pkg /tmp/klick.pkg -target /
open -a 'kl!ck'
\`\`\`

Удаление на Mac: \`sudo '/Applications/kl!ck.app/Contents/Resources/uninstall.sh'\`

**iPhone и iPad:** в App Store kl!ck нет — \`klick-ios.ipa\` подписывает тот, кто ставит, и для VPN нужен
**платный** аккаунт разработчика Apple или купленный сертификат с правами VPN (с бесплатным Apple ID приложение
встанет, но VPN не включится). Проверено с [FlareStore](https://flarestore.app/guide/ios/). Источник для
FlareStore, SideStore, AltStore, Feather и ESign — kl!ck будет обновляться оттуда:
\`https://github.com/$repo/releases/latest/download/klick-altstore.json\`. Все способы —
[как поставить на iPhone и iPad](https://github.com/$repo/blob/ios/docs/ios-install.md).
NOTES
