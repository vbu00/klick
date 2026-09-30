#!/bin/bash
# Удаляет kl!ck: службу (вернув прокси и DNS), окно, автозапуск.
#   sudo '/Applications/kl!ck.app/Contents/Resources/uninstall.sh'          # подключения и настройки остаются
#   sudo '/Applications/kl!ck.app/Contents/Resources/uninstall.sh' --wipe   # всё, вместе с данными
# (одинарные кавычки: в zsh `!` внутри двойных — подстановка из истории команд)
set -u
[[ $EUID -eq 0 ]] || exec sudo "$0" "$@"
wipe=""
[[ "${1:-}" == "--wipe" ]] && wipe="--wipe"

/usr/bin/pkill -x klick 2>/dev/null || true

installed="/Library/PrivilegedHelperTools/klick/klick-service"
bundled="/Applications/kl!ck.app/Contents/MacOS/klick-service"
if [[ -x "$installed" ]]; then
    "$installed" uninstall $wipe
elif [[ -x "$bundled" ]]; then
    "$bundled" uninstall $wipe
else
    /bin/launchctl bootout system/app.klick.service 2>/dev/null || true
    rm -f /Library/LaunchDaemons/app.klick.service.plist
    # Правила Kill Switch в брандмауэре pf служба нарочно оставляет при остановке — снять.
    /sbin/pfctl -q -a com.apple/090.klick -F all 2>/dev/null || true
    rm -rf /Library/PrivilegedHelperTools/klick
    [[ -n "$wipe" ]] && rm -rf "/Library/Application Support/klick"
fi

# Автозапуск окна и данные окна у пользователей этого Mac.
for home in /Users/*; do
    [[ -d "$home/Library" ]] || continue
    agent="$home/Library/LaunchAgents/app.klick.desktop.plist"
    if [[ -f "$agent" ]]; then
        /bin/launchctl bootout "gui/$(/usr/bin/stat -f%u "$home")" "$agent" 2>/dev/null || true
        rm -f "$agent"
    fi
    if [[ -n "$wipe" ]]; then
        rm -rf "$home/Library/WebKit/app.klick.desktop" "$home/Library/Caches/app.klick.desktop" "$home/Library/Application Support/app.klick.desktop"
    fi
done

# Ссылки klick:// больше ничего не открывают: убрать программу из LaunchServices (у root и у того, кто за Mac).
app="/Applications/kl!ck.app"
lsregister="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
if [[ -d "$app" ]]; then
    "$lsregister" -u "$app" 2>/dev/null || true
    console="$(/usr/bin/stat -f%Su /dev/console)"
    if [[ -n "$console" && "$console" != "root" && "$console" != "loginwindow" ]]; then
        /bin/launchctl asuser "$(/usr/bin/id -u "$console")" /usr/bin/sudo -u "$console" "$lsregister" -u "$app" 2>/dev/null || true
    fi
fi
rm -rf "$app"
/usr/sbin/pkgutil --forget app.klick.pkg >/dev/null 2>&1 || true
echo "kl!ck удалён${wipe:+ вместе с данными}"
