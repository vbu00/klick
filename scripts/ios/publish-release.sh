#!/bin/bash
# Выложить kl!ck для iOS в Releases: klick-ios.ipa и источник klick-altstore.json — для постоянных ссылок
#   https://github.com/<владелец>/<репозиторий>/releases/latest/download/klick-ios.ipa
#   https://github.com/<владелец>/<репозиторий>/releases/latest/download/klick-altstore.json
# Версия — из Cargo.toml. Если выпуска v<версия> ещё нет, он создаётся на этом коммите с общим текстом
# (scripts/release-notes.sh); если есть (с установщиками для Windows и macOS) — файлы добавляются в него.
# Запускает workflow после сборки и проверок (нужны GH_TOKEN и GITHUB_REPOSITORY):
#   scripts/ios/publish-release.sh
set -euo pipefail
cd "$(dirname "$0")/../.."

ver="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
tag="v$ver"
repo="${GITHUB_REPOSITORY:?нужен GITHUB_REPOSITORY (владелец/репозиторий)}"
commit="${GITHUB_SHA:-$(git rev-parse HEAD)}"
for f in dist/klick-ios.ipa dist/klick-altstore.json; do
    [[ -f "$f" ]] || { echo "нет $f — сначала scripts/ios/build.sh и scripts/ios/altstore-source.sh" >&2; exit 1; }
done
if gh release view "$tag" --repo "$repo" >/dev/null 2>&1; then
    if gh release view "$tag" --repo "$repo" --json assets --jq '.assets[].name' | grep -qx 'klick-ios.ipa'; then
        echo "в выпуске $tag kl!ck для iOS уже есть — поднимите версию в Cargo.toml" >&2
        exit 1
    fi
    gh release upload "$tag" dist/klick-ios.ipa dist/klick-altstore.json --repo "$repo"
    echo "kl!ck для iOS добавлен в выпуск: https://github.com/$repo/releases/tag/$tag"
    exit 0
fi

notes="$(mktemp)"
trap 'rm -f "$notes"' EXIT
scripts/release-notes.sh "$ver" "$repo" > "$notes"
gh release create "$tag" dist/klick-ios.ipa dist/klick-altstore.json --repo "$repo" --target "$commit" \
    --title "kl!ck $ver" --notes-file "$notes"
echo "выпуск: https://github.com/$repo/releases/tag/$tag"
