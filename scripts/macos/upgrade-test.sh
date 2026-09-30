#!/bin/bash
# Обновление поверх прошлой версии, как у человека: ставим прошлый выпуск из Releases, подключаемся,
# ставим новый пакет поверх — служба обновилась, подключения и настройки на месте, VPN вернулся сам.
# Прошлый выпуск — самый свежий в Releases с пакетом для macOS (кроме проверяемой версии); если такого
# ещё нет, проверять не с чего, и скрипт выходит без ошибки.
# Меняет настройки сети Mac — запускать на тестовой машине или в CI.
#
#   sudo GITHUB_REPOSITORY=владелец/репозиторий scripts/macos/upgrade-test.sh dist/klick-<версия>.pkg [тег прошлого выпуска]
set -uo pipefail

[[ $EUID -eq 0 ]] || { echo "нужен root: sudo $0 $*" >&2; exit 2; }
new_pkg="${1:?путь к новому .pkg}"
[[ -f "$new_pkg" ]] || { echo "нет $new_pkg" >&2; exit 2; }
repo="${GITHUB_REPOSITORY:?нужен GITHUB_REPOSITORY (владелец/репозиторий с выпусками)}"
new_ver="$(basename "$new_pkg" .pkg)"; new_ver="${new_ver#klick-}"
old_tag="${2:-}"
if [[ -z "$old_tag" ]]; then
    auth=()
    [[ -n "${GH_TOKEN:-}" ]] && auth=(-H "Authorization: Bearer $GH_TOKEN")
    releases="$(curl -fsSL ${auth[@]+"${auth[@]}"} "https://api.github.com/repos/$repo/releases?per_page=50")" \
        || { echo "не прочитать список выпусков $repo" >&2; exit 1; }
    old_tag="$(printf '%s' "$releases" | /usr/bin/python3 -c '
import json, sys
for r in json.load(sys.stdin):
    if not r.get("draft") and r["tag_name"] != sys.argv[1] and any(a["name"] == "klick-macos.pkg" for a in r.get("assets", [])):
        print(r["tag_name"])
        break
' "v$new_ver")"
fi
if [[ -z "$old_tag" ]]; then
    echo "== в $repo нет прошлого выпуска с пакетом для macOS — обновлять не с чего, пропускаю"
    exit 0
fi
app="/Applications/kl!ck.app"
svc="/Library/PrivilegedHelperTools/klick/klick-service"
cli() { "$app/Contents/MacOS/klick-cli" --prod "$@"; }
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
state_is() { cli status | grep -q "\"vpn\": \"$1\""; }
version_is() { cli about | grep -q "\"version\": \"$1\""; }
install_pkg() { installer -pkg "$1" -target / >"$work/installer.log" 2>&1 || { tail -20 "$work/installer.log"; return 1; }; }
quit_app() { pkill -x klick 2>/dev/null; true; }

cleanup() {
    echo "== уборка"
    [[ -n "${server_pid:-}" ]] && kill "$server_pid" 2>/dev/null
    quit_app
    [[ -x "$svc" ]] && "$svc" uninstall --wipe >/dev/null 2>&1
    rm -rf "$app" "$work"
}
trap cleanup EXIT

echo "== $(sw_vers -productName) $(sw_vers -productVersion) $(uname -m): $old_tag → $new_ver"

echo "== прошлая версия $old_tag из Releases"
curl -fsSL -o "$work/old.pkg" "https://github.com/$repo/releases/download/$old_tag/klick-macos.pkg" || { echo "не скачать $old_tag"; exit 1; }
check "установилась" install_pkg "$work/old.pkg"
quit_app
check "служба отвечает" wait_for 20 cli status
check "версия ${old_tag#v}" version_is "${old_tag#v}"

# Тестовый сервер: второе ядро с socks5 на 127.0.0.1:21080, выходит в интернет мимо kl!ck.
iface="$(route -n get default 2>/dev/null | awk '/interface:/ {print $2}')"
cp /Library/PrivilegedHelperTools/klick/mihomo "$work/mihomo-server"
cat > "$work/server.yaml" <<YAML
mixed-port: 0
log-level: warning
interface-name: ${iface:-en0}
listeners:
  - name: s5
    type: socks
    port: 21080
    listen: 127.0.0.1
    udp: true
mode: rule
rules:
  - MATCH,DIRECT
YAML
"$work/mihomo-server" -d "$work" -f "$work/server.yaml" > "$work/server.log" 2>&1 &
server_pid=$!
check "тестовый сервер слушает 21080" wait_for 10 nc -z 127.0.0.1 21080

check "добавить ссылку" cli add "socks5://127.0.0.1:21080#Upgrade"
cli routing all >/dev/null
cli mode proxy >/dev/null
cli connect >/dev/null
check "подключено на старой версии" wait_for 20 state_is connected

echo "== обновление до $new_ver поверх"
check "новый пакет установился поверх" install_pkg "$new_pkg"
quit_app
check "служба отвечает" wait_for 30 cli status
check "версия $new_ver" version_is "$new_ver"
kept() { cli settings | grep -q 'Upgrade'; }
routing_kept() { cli status | grep -q '"routing": "all_vpn"'; }
check "подключение осталось" kept
check "положение «Всё через VPN» осталось" routing_kept
check "VPN вернулся сам после обновления" wait_for 40 state_is connected
check "системный прокси на месте" bash -c 'scutil --proxy | grep -q "HTTPPort : 7890"'
check "ядро от старой версии не осталось" test "$(pgrep -f '/Library/PrivilegedHelperTools/klick/mihomo' | wc -l | tr -d ' ')" = "1"
cli disconnect >/dev/null
check "выключено" wait_for 10 state_is off
check "прокси снят" wait_for 5 bash -c '! scutil --proxy | grep -q "HTTPPort : 7890"'

if [[ $failed -gt 0 ]]; then
    echo "--- журнал службы"
    tail -80 "/Library/Application Support/klick/logs/service.log" 2>/dev/null
    echo "== не прошло проверок: $failed"
    exit 1
fi
echo "== все проверки прошли"
