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
data="/Library/Application Support/klick"
no_binary=""

# Запасной путь вернуть DNS и прокси без службы (нет бинарников; настройки сети были заняты и отметки остались).
# Через networksetup и только то, что ставил kl!ck: DNS 198.18.0.2 и прокси 127.0.0.1:7890. Чужие значения не трогаем.
# Работает с сетевыми службами текущего размещения (Location); остальные возвращает служба kl!ck.
# $1 = fix — вернуть, check — только посчитать. Печатает, сколько наших значений нашлось.
scan_network() {
    local ns=/usr/sbin/networksetup svc out kind dns found=0
    [[ -x "$ns" ]] || { echo 0; return; }
    while IFS= read -r svc; do
        svc="${svc#\*}"   # выключенные службы в списке начинаются со «*»
        [[ -n "$svc" ]] || continue
        out="$("$ns" -getdnsservers "$svc" 2>/dev/null)"
        if [[ "$out" == "198.18.0.2" ]]; then
            found=$((found + 1))
            # Свои адреса человека — из резерва службы; нет их — DNS от роутера (DHCP).
            dns="$(saved_dns "$svc")"
            # shellcheck disable=SC2086 # адреса — отдельными словами
            [[ "$1" == fix ]] && "$ns" -setdnsservers "$svc" ${dns:-Empty} >/dev/null 2>&1
        fi
        for kind in webproxy securewebproxy socksfirewallproxy; do
            out="$("$ns" "-get$kind" "$svc" 2>/dev/null | /usr/bin/awk -F': ' '$1=="Enabled"{e=$2} $1=="Server"{s=$2} $1=="Port"{p=$2} END{print e" "s" "p}')"
            if [[ "$out" == "Yes 127.0.0.1 7890" ]]; then
                found=$((found + 1))
                [[ "$1" == fix ]] && "$ns" "-set${kind}state" "$svc" off >/dev/null 2>&1
            fi
        done
    done < <("$ns" -listallnetworkservices 2>/dev/null | /usr/bin/tail -n +2)
    echo "$found"
}

markers_left() { [[ -e "$data/dns.json" || -e "$data/sysproxy.json" ]]; }

# ID сетевой службы по её имени: резервы в dns.json лежат по ID (отметки старых версий — по имени).
service_id() {
    local id
    while IFS= read -r id; do
        [[ "$(printf 'show Setup:/Network/Service/%s\n' "$id" | /usr/sbin/scutil | /usr/bin/awk -F' : ' '$1 ~ /UserDefinedName$/ {print $2; exit}')" == "$1" ]] && { echo "$id"; return; }
    done < <(printf 'list Setup:/Network/Service/[^/]+$\n' | /usr/sbin/scutil | /usr/bin/sed -n 's|.*Setup:/Network/Service/||p')
}

# Адреса DNS службы до kl!ck из резерва, через пробел; пусто — были от роутера или резерва нет.
saved_dns() {
    local key out
    [[ -f "$data/dns.json" ]] || return
    for key in "$(service_id "$1")" "$1"; do
        [[ -n "$key" ]] || continue
        out="$(/usr/bin/plutil -extract "services.$key" json -o - "$data/dns.json" 2>/dev/null)" || continue
        printf '%s' "$out" | /usr/bin/tr -d '[]" ' | /usr/bin/tr ',' ' '
        return
    done
}

if [[ -x "$installed" ]]; then
    "$installed" uninstall $wipe
elif [[ -x "$bundled" ]]; then
    "$bundled" uninstall $wipe
else
    no_binary=1
    /bin/launchctl bootout system/app.klick.service 2>/dev/null || true
    rm -f /Library/LaunchDaemons/app.klick.service.plist
    # Правила Kill Switch в брандмауэре pf служба нарочно оставляет при остановке — снять.
    /sbin/pfctl -q -a com.apple/090.klick -F all 2>/dev/null || true
    # Служба упала с включённым VPN: её ядра (VPN и страж Kill Switch) живут без неё и держат TUN.
    /usr/bin/pkill -9 -f '^/Library/PrivilegedHelperTools/klick/' 2>/dev/null || true
    rm -rf /Library/PrivilegedHelperTools/klick
fi

# Службы нет или она не смогла вернуть DNS и прокси (отметки остались) — вернуть самим.
if [[ -n "$no_binary" ]] || markers_left; then
    scan_network fix >/dev/null
    if [[ "$(scan_network check)" == 0 ]]; then
        rm -f "$data/dns.json" "$data/sysproxy.json"
        /usr/bin/dscacheutil -flushcache 2>/dev/null || true
        /usr/bin/killall -HUP mDNSResponder 2>/dev/null || true
    else
        echo "ВНИМАНИЕ: DNS или прокси kl!ck вернуть не удалось: Системные настройки → Сеть → DNS (убрать 198.18.0.2) и Прокси (выключить 127.0.0.1)." >&2
    fi
fi
# Данные — после возврата сети; отметки, по которым ещё не вернули, не стираем.
if [[ -n "$wipe" && -d "$data" ]]; then
    if markers_left; then
        /usr/bin/find "$data" -mindepth 1 -maxdepth 1 ! -name dns.json ! -name sysproxy.json -exec rm -rf {} +
    else
        rm -rf "$data"
    fi
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
