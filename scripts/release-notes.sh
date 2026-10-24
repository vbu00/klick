#!/bin/bash
# Текст выпуска для Windows и macOS сразу: что нового (docs/release-notes.md) и как поставить.
# Выпуск один на версию — тег v<версия>, в нём и установщик для Windows, и пакет для Mac; текст пишет
# тот, кто выкладывает первым (scripts/macos/publish-release.sh или выпуск в .github/workflows/windows.yml).
#   scripts/release-notes.sh <версия> <владелец/репозиторий>
set -euo pipefail
cd "$(dirname "$0")/.."
ver="${1:?нужна версия}"
repo="${2:?нужен владелец/репозиторий}"
cat <<NOTES
kl!ck $ver для Windows 10/11 x64 и macOS 12+ (Apple Silicon и Intel).

$(cat docs/release-notes.md)

**Windows:** скачать \`klick-$ver-x64-setup.exe\` ниже и запустить — поверх прежней kl!ck подключения и настройки остаются.

**macOS:** пакет не подписан сертификатом Apple, поэтому ставится из Терминала (так без вопроса Gatekeeper):

\`\`\`sh
curl -fL -o /tmp/klick.pkg https://github.com/$repo/releases/download/v$ver/klick-macos.pkg
sudo installer -pkg /tmp/klick.pkg -target /
open -a 'kl!ck'
\`\`\`

Удаление на Mac: \`sudo '/Applications/kl!ck.app/Contents/Resources/uninstall.sh'\`
NOTES
