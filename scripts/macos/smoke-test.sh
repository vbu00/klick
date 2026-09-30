#!/bin/bash
# Сквозная проверка kl!ck на настоящем Mac: ставит службу из kl!ck.app, поднимает тестовый «VPN-сервер»
# (второй mihomo с socks5 на 127.0.0.1:21080) и проходит сценарии: прокси, TUN с подменой DNS,
# Kill Switch, падение службы, удаление. Меняет настройки сети Mac и в конце возвращает их —
# запускать на тестовой машине или в CI (GitHub Actions, macOS), а не на рабочем компьютере.
#
#   sudo scripts/macos/smoke-test.sh 'dist/kl!ck.app'
set -uo pipefail

[[ $EUID -eq 0 ]] || { echo "нужен root: sudo $0 $*" >&2; exit 2; }
app="${1:-dist/kl!ck.app}"
[[ -x "$app/Contents/MacOS/klick-service" ]] || { echo "нет $app" >&2; exit 2; }
app="$(cd "$(dirname "$app")" && pwd)/$(basename "$app")"

svc="/Library/PrivilegedHelperTools/klick/klick-service"
core="/Library/PrivilegedHelperTools/klick/mihomo"
cli() { "$app/Contents/MacOS/klick-cli" --prod "$@"; }
work="$(mktemp -d)"
log="/Library/Application Support/klick/logs/service.log"
failed=0
probe="https://www.gstatic.com/generate_204"

pass() { echo "  ✓ $*"; }
fail() { echo "  ✗ $*"; failed=$((failed + 1)); }
check() { # check "описание" команда…
    local what="$1"; shift
    if "$@" >/dev/null 2>&1; then pass "$what"; else fail "$what"; fi
}
wait_for() { # wait_for секунд команда…
    local t="$1"; shift
    for _ in $(seq 1 $((t * 4))); do "$@" >/dev/null 2>&1 && return 0; sleep 0.25; done
    return 1
}
state_is() { cli status | grep -q "\"vpn\": \"$1\""; }
http_with() { local bin="$1"; shift; [[ "$("$bin" -s -o /dev/null -m 10 -w '%{http_code}' "$@" "$probe")" == "204" ]]; }
http_ok() { http_with /usr/bin/curl "$@"; }
first_service() { networksetup -listallnetworkservices | sed 1d | grep -v '^\*' | head -1; }
proxy_on() { scutil --proxy | grep -q "HTTPEnable : 1" && scutil --proxy | grep -q "HTTPPort : 7890"; }
dns_ours() { scutil --dns | grep -q "nameserver\[0\] : 198.18.0.2"; }
tun_up() { ifconfig | grep -q "inet 198.18.0.1 "; }
pf_on() { pfctl -a com.apple/090.klick -s rules 2>/dev/null | grep -q "block return out quick all"; }
pf_enabled() { pfctl -s info 2>/dev/null | grep -q "Status: Enabled"; }
ks_has() { cli ks status | grep -q "\"folder\": \"$1\""; }
enabled_services() { networksetup -listallnetworkservices | sed 1d | grep -v '^\*'; }
# Настройки всех сетевых служб, как их видит networksetup: kl!ck пишет их через SystemConfiguration,
# networksetup читает независимо — сверяем с тем, что было до подключения.
netconf_snapshot() {
    networksetup -listallnetworkservices | sed 1d | sed 's/^\*//' | while IFS= read -r s; do
        echo "[$s]"
        for f in -getwebproxy -getsecurewebproxy -getsocksfirewallproxy -getproxybypassdomains -getautoproxyurl -getproxyautodiscovery -getdnsservers; do
            networksetup "$f" "$s"
        done
    done
}
netconf_same() {
    netconf_snapshot > "$work/netconf.now"
    diff "$work/netconf.before" "$work/netconf.now" >&2
}
# Прокси kl!ck у каждой включённой службы, а не только у основной.
all_proxied() {
    local s
    while IFS= read -r s; do
        for f in -getwebproxy -getsecurewebproxy -getsocksfirewallproxy; do
            networksetup "$f" "$s" | tr '\n' ' ' | grep -q "Enabled: Yes Server: 127.0.0.1 Port: 7890" || return 1
        done
        networksetup -getproxybypassdomains "$s" | grep -qx "192.168.0.0/16" || return 1
    done < <(enabled_services)
}
all_dns_ours() {
    local s
    while IFS= read -r s; do
        [[ "$(networksetup -getdnsservers "$s")" == "198.18.0.2" ]] || return 1
    done < <(enabled_services)
}
# Сколько секунд заняла команда (с долями).
seconds() { local TIMEFORMAT=%R; { time "$@" >/dev/null 2>&1; } 2>&1; }
quick() { awk -v t="$1" -v max="$2" 'BEGIN { exit !(t <= max) }'; }
disconnect_quickly() { # disconnect_quickly "режим"
    local took
    took="$(seconds cli disconnect)"
    echo "    «Отключить» ($1): $took с; $(grep 'отключено за' "$log" | tail -1 | sed 's/.*отключено/отключено/')"
    check "«Отключить» ($1) быстрее 2 с" quick "$took" 2
}

# Проверки Kill Switch — от имени человека, а не root: root правило pf пропускает (это ядро kl!ck).
user="${SUDO_USER:-nobody}"
[[ "$user" == "root" ]] && user=nobody
ks="/Users/Shared/klick-ks/tool/kscurl"
as_user() { sudo -u "$user" "$@"; }
user_ok() { [[ "$(as_user /usr/bin/curl -s -o /dev/null -m 8 -w '%{http_code}' "$probe")" == "204" ]]; }
ks_ok() { [[ "$(as_user "$ks" -s -o /dev/null -m 8 -w '%{http_code}' "$probe")" == "204" ]]; }
# Защищённая программа ни разу не вышла в интернет за столько-то секунд — пока идёт сбой.
ks_never() {
    local end=$((SECONDS + $1))
    while (( SECONDS < end )); do
        if [[ "$(as_user "$ks" -s -o /dev/null -m 2 -w '%{http_code}' "$probe" 2>/dev/null)" == "204" ]]; then
            echo "    защищённая программа вышла в интернет на $((SECONDS - end + $1))-й секунде" >&2
            return 1
        fi
        sleep 0.2
    done
}
start_server() {
    "$work/mihomo-server" -d "$work" -f "$work/server.yaml" > "$work/server.log" 2>&1 &
    server_pid=$!
    wait_for 10 nc -z 127.0.0.1 21080 && return 0
    echo "--- журнал тестового сервера" >&2
    tail -20 "$work/server.log" >&2
    return 1
}

cleanup() {
    echo "== уборка"
    [[ -n "${server_pid:-}" ]] && kill "$server_pid" 2>/dev/null
    [[ -x "$svc" ]] && "$svc" uninstall --wipe >/dev/null 2>&1
    if [[ -n "${user_service:-}" ]]; then
        networksetup -setdnsservers "$user_service" ${user_dns:-Empty}
        networksetup -setwebproxystate "$user_service" off
        networksetup -setproxybypassdomains "$user_service" Empty
    fi
    rm -rf "$work" /Users/Shared/klick-ks /Users/Shared/klick-ks-link
}
trap cleanup EXIT

echo "== $(sw_vers -productName) $(sw_vers -productVersion) $(uname -m)"
echo "   сетевые службы: $(enabled_services | tr '\n' ';'); DNS: $(networksetup -getdnsservers "$(first_service)" | tr '\n' ' ')"
# Свои настройки у человека: DNS вручную, выключенный прокси, исключения. kl!ck должен вернуть их
# как было. DNS — тот же, что раздал роутер, чтобы интернет у раннера работал как прежде.
user_service="$(first_service)"
user_dns="$(networksetup -getdnsservers "$user_service" | grep -E '^[0-9a-f.:]+$' | tr '\n' ' ')"
router_dns="$(scutil --dns | awk '/nameserver\[0\]/ {print $3; exit}')"
networksetup -setdnsservers "$user_service" "${router_dns:-8.8.8.8}" 8.8.8.8
networksetup -setwebproxy "$user_service" 10.255.255.1 3128
networksetup -setwebproxystate "$user_service" off
networksetup -setproxybypassdomains "$user_service" "*.klick.test" 10.255.0.0/16
netconf_snapshot > "$work/netconf.before"
# Физический интерфейс — до того, как kl!ck поднимет TUN.
iface="$(route -n get default 2>/dev/null | awk '/interface:/ {print $2}')"

echo "== установка службы"
# Метка карантина, как у программы из скачанного пакета: установка должна снять её сама, без /usr/bin/xattr.
xattr -w com.apple.quarantine "0083;00000000;Safari;" "$app/Contents/Info.plist"
"$app/Contents/MacOS/klick-service" install || { echo "install не удался"; exit 1; }
check "метка карантина с программы снята" bash -c "! xattr -p com.apple.quarantine '$app/Contents/Info.plist'"
check "служба в launchd" launchctl print system/app.klick.service
check "сокет /var/run/klick.sock появился" wait_for 15 test -S /var/run/klick.sock
check "сокет root:staff 660" test "$(stat -f '%Su:%Sg %Lp' /var/run/klick.sock)" = "root:staff 660"
check "папка данных только для root" test "$(stat -f '%Su %Lp' '/Library/Application Support/klick')" = "root 700"
check "служба отвечает" wait_for 10 cli status
cli about

echo "== тестовый сервер"
# Сервер ведёт себя как удалённый: выходит в интернет через физический интерфейс мимо TUN kl!ck и сам
# разрешает имена. Иначе, запущенный при включённом TUN, он запомнит подменные адреса 198.18.x.x
# из DNS kl!ck, и после отключения VPN они никуда не ведут.
cat > "$work/server.yaml" <<YAML
mixed-port: 0
log-level: warning
interface-name: ${iface:-en0}
dns:
  enable: true
  ipv6: false
  nameserver:
    - 8.8.8.8
    - https://1.1.1.1/dns-query
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
# Копия ядра: по пути установленного ядра проверка ниже считает ядра службы.
cp "$core" "$work/mihomo-server"
start_server && pass "тестовый сервер слушает 21080" || fail "тестовый сервер слушает 21080"

echo "== подключение и серверы"
check "добавить ссылку" cli add "socks5://127.0.0.1:21080#CI"
check "серверы (проверочное ядро)" cli servers
# Первое соединение раннера наружу бывает дольше 5 с (тайм-аут проверки) — как человек, жмём ещё раз.
latency_ok() { cli latency | grep -q '"delay": [0-9]'; }
check "задержка измерена" wait_for 30 latency_ok

echo "== режим «Системный прокси»"
cli mode proxy >/dev/null
cli connect >/dev/null
check "подключено" wait_for 20 state_is connected
check "системный прокси 127.0.0.1:7890" proxy_on
check "прокси у всех включённых сетевых служб" all_proxied
check "страница через порт 7890" http_ok -x http://127.0.0.1:7890
cli ip | head -8
disconnect_quickly "Системный прокси"
check "выключено" state_is off
check "прокси снят" wait_for 5 bash -c '! scutil --proxy | grep -q "HTTPPort : 7890"'
check "настройки сети как до подключения" netconf_same

echo "== режим VPN (TUN)"
cli mode tun >/dev/null
cli connect >/dev/null
check "подключено" wait_for 20 state_is connected
check "адаптер utun с 198.18.0.1" tun_up
check "DNS подменён на 198.18.0.2" dns_ours
check "DNS подменён у всех включённых сетевых служб" all_dns_ours
check "имя отвечает подменным адресом" bash -c 'dscacheutil -q host -a name www.gstatic.com | grep -q "ip_address: 198.18."'
check "страница через TUN" http_ok
cli ip | grep -E '"dns_protected"|"ipv4"' | head -3
# Смена режима на ходу: VPN переподключается в «Системный прокси», адаптер и подмена DNS уходят.
cli mode proxy >/dev/null
check "смена режима на ходу: подключено" wait_for 20 state_is connected
check "смена режима на ходу: системный прокси стоит" wait_for 10 proxy_on
check "смена режима на ходу: адаптер убран" wait_for 10 bash -c '! ifconfig | grep -q "inet 198.18.0.1 "'
check "смена режима на ходу: DNS возвращён" bash -c '! scutil --dns | grep -q "nameserver\[0\] : 198.18.0.2"'
cli disconnect >/dev/null
check "настройки сети как до подключения" netconf_same
cli mode tun >/dev/null
cli connect >/dev/null
check "VPN (TUN) снова подключён" wait_for 20 state_is connected
disconnect_quickly "VPN (TUN)"
check "адаптер убран" wait_for 10 bash -c '! ifconfig | grep -q "inet 198.18.0.1 "'
check "DNS возвращён" wait_for 5 bash -c '! scutil --dns | grep -q "nameserver\[0\] : 198.18.0.2"'
check "настройки сети как до подключения" netconf_same
check "страница напрямую после VPN" http_ok

echo "== Kill Switch (проверки от имени $user)"
mkdir -p /Users/Shared/klick-ks/tool
cp /usr/bin/curl "$ks"
chmod 755 /Users/Shared/klick-ks /Users/Shared/klick-ks/tool "$ks"
cli mode tun >/dev/null
ln -sfn /Users/Shared/klick-ks/tool /Users/Shared/klick-ks-link
check "программа до Kill Switch ходит напрямую" ks_ok
check "добавить в Kill Switch по символьной ссылке" cli ks add /Users/Shared/klick-ks-link
check "в Kill Switch записан настоящий путь, а не ссылка" ks_has /Users/Shared/klick-ks/tool
check "правила pf стоят" wait_for 10 pf_on
check "главный набор pf спрашивает якоря Apple" bash -c 'pfctl -s rules 2>/dev/null | grep -q "anchor \"com.apple/\*\""'
check "страж поднял адаптер при выключенном VPN" wait_for 15 tun_up
check "защищённая программа без VPN не выходит" ks_never 4
check "остальные ходят напрямую" user_ok

echo "== Kill Switch: программа пропала с диска (обновление, внешний диск)"
mv /Users/Shared/klick-ks/tool /Users/Shared/klick-ks/tool.away
sleep 12
check "программы нет — правила pf на месте" pf_on
check "программы нет — страж на месте" tun_up
mv /Users/Shared/klick-ks/tool.away /Users/Shared/klick-ks/tool
check "программа вернулась — сразу защищена" ks_never 3

echo "== Kill Switch: pf выключила другая программа"
pfctl -d >/dev/null 2>&1
check "pf выключен — защищённая программа всё равно не выходит (страж)" ks_never 3
check "служба включила pf снова" wait_for 25 pf_enabled
check "правила pf снова стоят" pf_on

echo "== Kill Switch: сбои при выключенном VPN"
kill -9 "$(cat /var/run/klick/guard.sock.pid)"
check "упал страж — защищённая программа не выходит" ks_never 6
check "страж вернулся" wait_for 30 tun_up
check "остальные снова ходят" wait_for 30 user_ok
kill -9 "$(launchctl print system/app.klick.service | awk '/pid =/ {print $3}')"
check "упала служба — защищённая программа не выходит" ks_never 10
check "правила pf пережили падение службы" pf_on
check "служба вернулась" wait_for 30 cli status
check "страж вернулся после перезапуска службы" wait_for 30 tun_up
check "остальные снова ходят" wait_for 30 user_ok

echo "== Kill Switch: сбои при включённом VPN (TUN)"
cli connect >/dev/null
check "VPN подключён" wait_for 20 state_is connected
check "защищённая программа ходит через VPN" wait_for 20 ks_ok
kill "$server_pid"; wait "$server_pid" 2>/dev/null
check "упал сервер VPN — защищённая программа не выходит напрямую" ks_never 6
check "остальные ходят (положение «только выбранное»)" user_ok
kill -9 "$(cat /var/run/klick/core.sock.pid)"
check "упало ядро при мёртвом сервере — защищённая программа не выходит" ks_never 10
start_server && pass "сервер снова работает" || fail "сервер снова работает"
check "защищённая программа снова ходит через VPN" wait_for 60 ks_ok
cli disconnect >/dev/null
check "после отключения защищённая программа не выходит" ks_never 4

echo "== Kill Switch выключен"
check "убрать из Kill Switch" cli ks rm /Users/Shared/klick-ks/tool
check "правила pf сняты" wait_for 10 bash -c '! pfctl -a com.apple/090.klick -s rules 2>/dev/null | grep -q block'
check "страж остановлен" wait_for 10 bash -c '! ifconfig | grep -q "inet 198.18.0.1 "'
check "программа снова ходит напрямую" wait_for 10 ks_ok
rm -rf /Users/Shared/klick-ks /Users/Shared/klick-ks-link

echo "== падение службы"
cli mode proxy >/dev/null
cli connect >/dev/null
check "подключено" wait_for 20 state_is connected
pid="$(launchctl print system/app.klick.service | awk '/pid =/ {print $3}')"
kill -9 "$pid"
check "launchd перезапустил службу, VPN вернулся" wait_for 30 state_is connected
check "ядро от упавшей службы не осталось" test "$(pgrep -f '/Library/PrivilegedHelperTools/klick/mihomo' | wc -l | tr -d ' ')" = "1"
check "прокси на месте" proxy_on
cli disconnect >/dev/null

echo "== сервер пропал: окно не ждёт службу"
cli connect >/dev/null
check "подключено" wait_for 20 state_is connected
kill "$server_pid" 2>/dev/null
server_lost() { cli status | grep -Eq '"vpn": "(reconnecting|server_down)"'; }
check "служба заметила, что сервер не отвечает" wait_for 60 server_lost
started=$(date +%s)
cli disconnect >/dev/null
took=$(( $(date +%s) - started ))
echo "    «Отключить» во время переподключения: $took с"
check "«Отключить» срабатывает сразу, даже во время переподключения" test "$took" -le 3
check "выключено" state_is off

if [[ $failed -gt 0 ]]; then
    echo "--- журнал службы"
    tail -120 "$log" 2>/dev/null
    echo "--- журнал тестового сервера"
    tail -30 "$work/server.log" 2>/dev/null
fi

echo "== удаление"
"$svc" uninstall --wipe
check "служба убрана из launchd" bash -c '! launchctl print system/app.klick.service'
check "файлы службы удалены" test ! -e /Library/PrivilegedHelperTools/klick
check "прокси не остался" bash -c '! scutil --proxy | grep -q "HTTPPort : 7890"'
check "DNS не остался" bash -c '! scutil --dns | grep -q "198.18.0.2"'
check "правил pf не осталось" bash -c '! pfctl -a com.apple/090.klick -s rules 2>/dev/null | grep -q block'
check "интернет у пользователя есть" user_ok
check "настройки сети как до установки" netconf_same

if [[ $failed -gt 0 ]]; then
    echo "== не прошло проверок: $failed"
    exit 1
fi
echo "== все проверки прошли"
