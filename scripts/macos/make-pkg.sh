#!/bin/bash
# Установщик .pkg: kl!ck.app в «Программы» и служба в launchd (postinstall → `klick-service install`).
#   scripts/macos/make-pkg.sh <kl!ck.app> <версия> <arm64|x86_64|arm64,x86_64> <выход.pkg>
set -euo pipefail
app="$1"; version="$2"; arch="$3"; out="$4"
here="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

mkdir -p "$work/root/Applications" "$work/scripts"
ditto "$app" "$work/root/Applications/kl!ck.app"
cp "$here/pkg/preinstall" "$here/pkg/postinstall" "$work/scripts/"
chmod 755 "$work/scripts/"*

# Пакет не должен «переезжать»: если где-то на диске уже лежит kl!ck.app, Installer по умолчанию
# обновил бы ту копию, а не поставил программу в «Программы».
pkgbuild --analyze --root "$work/root" "$work/component.plist" >/dev/null
plutil -replace 0.BundleIsRelocatable -bool NO "$work/component.plist"
plutil -replace 0.BundleIsVersionChecked -bool NO "$work/component.plist"

pkgbuild --root "$work/root" --component-plist "$work/component.plist" --identifier app.klick.pkg \
    --version "$version" --install-location / --scripts "$work/scripts" "$work/klick.pkg"

sed -e "s/@VERSION@/$version/g" -e "s/@ARCH@/$arch/g" "$here/pkg/distribution.xml" > "$work/distribution.xml"
mkdir -p "$(dirname "$out")"
# Пустой массив под `set -u` в bash 3.2 (он в macOS) — ошибка, поэтому две ветки.
if [[ -n "${KLICK_INSTALLER_IDENTITY:-}" ]]; then
    productbuild --distribution "$work/distribution.xml" --package-path "$work" --sign "$KLICK_INSTALLER_IDENTITY" "$out"
else
    productbuild --distribution "$work/distribution.xml" --package-path "$work" "$out"
fi
