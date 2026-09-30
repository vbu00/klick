#!/bin/bash
# Ссылки klick://add на Mac, как у человека: ставим .pkg — схема klick сразу ведёт в kl!ck.app;
# ссылка открывает kl!ck, следующие ссылки приходят в ту же копию; сама по себе ссылка ничего не
# добавляет, её нет в журналах службы и окна; после удаления схема больше никуда не ведёт.
# Нужен человек за Mac (в CI — пользователь раннера): ссылки открываются от его имени.
#
#   sudo scripts/macos/deeplink-test.sh dist/klick-<версия>.pkg
set -uo pipefail

[[ $EUID -eq 0 ]] || { echo "нужен root: sudo $0 $*" >&2; exit 2; }
pkg="${1:?путь к .pkg}"
[[ -f "$pkg" ]] || { echo "нет $pkg" >&2; exit 2; }
app="/Applications/kl!ck.app"
cli() { "$app/Contents/MacOS/klick-cli" --prod "$@"; }
log="/Library/Application Support/klick/logs/service.log"
work="$(mktemp -d)"
failed=0

pass() { echo "  ✓ $*"; }
fail() { echo "  ✗ $*"; failed=$((failed + 1)); }
check() {
    local what="$1"; shift
    if "$@" >/dev/null 2>&1; then pass "$what"; else fail "$what"; fi
}
wait_for() {
    local t="$1"; shift
    for _ in $(seq 1 $((t * 4))); do "$@" >/dev/null 2>&1 && return 0; sleep 0.25; done
    return 1
}

user="$(/usr/bin/stat -f%Su /dev/console)"
if [[ -z "$user" || "$user" == "root" || "$user" == "loginwindow" ]]; then
    echo "== за Mac никого нет — ссылки открывать некому, пропускаю"
    exit 0
fi
uid="$(/usr/bin/id -u "$user")"
as_user() { /bin/launchctl asuser "$uid" /usr/bin/sudo -u "$user" "$@"; }
# Какая программа откроет ссылку — у пользователя, как её откроет браузер.
handler() {
    as_user /usr/bin/osascript -l JavaScript -e '
ObjC.import("AppKit");
var u = $.NSWorkspace.sharedWorkspace.URLForApplicationToOpenURL($.NSURL.URLWithString("klick://add"));
u.isNil() ? "" : u.path.js' 2>/dev/null
}
handler_is() { [[ "$(handler)" == "$1" ]]; }
not_ours() { [[ "$(handler)" != "$app" ]]; }
procs() { pgrep -x klick | wc -l | tr -d ' '; }
one_klick() { [[ "$(procs)" == "1" ]]; }
conns() { cli settings | /usr/bin/python3 -c 'import json, sys; print(len(json.load(sys.stdin)["connections"]))'; }
quit_app() { pkill -x klick 2>/dev/null; wait_for 10 bash -c '! pgrep -x klick'; true; }

cleanup() {
    echo "== уборка"
    quit_app
    [[ -x "$app/Contents/Resources/uninstall.sh" ]] && "$app/Contents/Resources/uninstall.sh" --wipe >/dev/null 2>&1
    rm -rf "$work"
}
trap cleanup EXIT

echo "== $(sw_vers -productName) $(sw_vers -productVersion) $(uname -m), за Mac: $user"
started="$(date '+%Y-%m-%d %H:%M:%S')"
check "пакет установился" installer -pkg "$pkg" -target /
check "схема klick в Info.plist" bash -c "plutil -extract CFBundleURLTypes json -o - '$app/Contents/Info.plist' | grep -q '\"klick\"'"
# postinstall открывает окно — закрыть: проверяем и запуск по ссылке.
quit_app
check "служба отвечает" wait_for 20 cli status
check "сразу после установки ссылку klick:// открывает kl!ck.app" wait_for 10 handler_is "$app"
echo "    klick:// → $(handler)"
before="$(conns)"

as_user /usr/bin/open 'klick://add?url=https%3A%2F%2Fsub.example.com%2Fs%2FSECRET-mac-one&name=CI'
check "ссылка запустила kl!ck" wait_for 30 one_klick
sleep 3
as_user /usr/bin/open 'klick://add/https://second.example.org/s/SECRET-mac-two'
as_user /usr/bin/open 'klick://add?url=javascript%3Aalert(1)'
as_user /usr/bin/open 'klick://settings'
sleep 4
check "следующие ссылки — в ту же копию, kl!ck работает" one_klick
echo "    процессов klick: $(procs)"
check "по ссылкам ничего не добавилось" test "$(conns)" = "$before"
check "ссылок подписки нет в журнале службы" bash -c "! grep -q 'SECRET-mac' '$log'"
# Журнал окна на Mac — системный (Console): пишет ли туда kl!ck что-то со ссылкой.
/usr/bin/log show --start "$started" --predicate 'process == "klick"' --style compact > "$work/ui.log" 2>/dev/null
check "ссылок подписки нет в журнале окна" bash -c "! grep -q 'SECRET-mac' '$work/ui.log'"
quit_app

echo "== удаление"
check "удаление" "$app/Contents/Resources/uninstall.sh"
check "программы нет" test ! -e "$app"
# Сборка в dist/ тоже kl!ck.app: LaunchServices может знать и её — главное, что не удалённая программа.
check "ссылки klick:// больше не ведут в удалённую kl!ck" wait_for 15 not_ours
echo "    klick:// → $(handler || true)"

if [[ $failed -gt 0 ]]; then
    echo "--- журнал службы"
    tail -40 "$log" 2>/dev/null | sed -E 's#https?://[^" ]+#<ссылка>#g'
    echo "== не прошло проверок: $failed"
    exit 1
fi
echo "== все проверки прошли"
