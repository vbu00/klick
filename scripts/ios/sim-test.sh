#!/bin/bash
# kl!ck в симуляторе iOS: приложение ставится и запускается, окно открывается, движок службы поднимается
# (в журнале — строка «данные: …»), снимок экрана — в dist/ios-sim.png.
# С KLICK_TEST_SUB (ссылка подписки) — ещё и серверы: движок добавляет подписку и меряет задержку через
# каждый сервер ядром mihomo в самом приложении (свои серверы для CI — scripts/ios/test-servers.sh).
# Туннель Network Extension симулятор не запускает — подключение VPN проверяется только на устройстве
# (docs/ios-port.md).
#   scripts/ios/sim-test.sh dist/klick-ios-sim.app
set -euo pipefail
app="${1:?путь к .app для симулятора}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
bundle_id="app.klick.ios"

device="$(xcrun simctl list devices available -j | jq -r '[.devices | to_entries[] | select(.key | test("iOS")) | .value[] | select(.name | test("^iPhone"))] | last | .udid')"
[[ -n "$device" && "$device" != null ]] || { echo "нет симулятора iPhone" >&2; exit 1; }
echo "симулятор: $(xcrun simctl list devices -j | jq -r --arg d "$device" '.devices[][] | select(.udid == $d) | .name')"
xcrun simctl boot "$device" 2>/dev/null || true
xcrun simctl bootstatus "$device" -b >/dev/null
xcrun simctl install "$device" "$app"
# Переменные с приставкой SIMCTL_CHILD_ simctl передаёт запущенному приложению.
SIMCTL_CHILD_KLICK_SELFTEST_SUB="${KLICK_TEST_SUB:-}" xcrun simctl launch "$device" "$bundle_id"

data="$(xcrun simctl get_app_container "$device" "$bundle_id" data)"
log="$data/Library/Application Support/klick/logs/service.log"
wait_for() { # wait_for <текст> <секунды>
    local i
    for ((i = 0; i < $2; i++)); do
        if grep -q "$1" "$log" 2>/dev/null; then return 0; fi
        sleep 1
    done
    return 1
}

bad=0
if wait_for "данные:" 60; then
    echo "ok   движок службы запустился"
else
    echo "FAIL движок службы не запустился за 60 с"; bad=1
fi
sleep 3
mkdir -p "$root/dist"
xcrun simctl io "$device" screenshot "$root/dist/ios-sim.png" >/dev/null && echo "снимок: dist/ios-sim.png"
if xcrun simctl spawn "$device" launchctl list | grep -q "$bundle_id"; then
    echo "ok   приложение работает"
else
    echo "FAIL приложение закрылось"; bad=1
fi

if [[ -n "${KLICK_TEST_SUB:-}" ]]; then
    if wait_for "самопроверка:" 180; then
        line="$(grep "самопроверка:" "$log" | tail -1)"
        echo "${line#*самопроверка: }"
        # Хоть один сервер подписки должен ответить через ядро в приложении.
        if [[ "$line" == *"ошибка"* || "$line" == *"неожиданный"* ]] || ! grep -qE ' [1-9][0-9]*/[0-9]+' <<<"$line"; then
            echo "FAIL ни один сервер подписки не ответил"; bad=1
        else
            echo "ok   серверы подписки отвечают через ядро в приложении"
        fi
        # KLICK_TEST_EXPECT="hysteria2 1/1;vless 1/1" — какие серверы обязаны ответить (свои серверы CI).
        IFS=';' read -ra expect <<<"${KLICK_TEST_EXPECT:-}"
        for e in ${expect[@]+"${expect[@]}"}; do
            if [[ "$line" == *"$e"* ]]; then echo "ok   $e"; else echo "FAIL ждали «$e»"; bad=1; fi
        done
    else
        echo "FAIL самопроверка с подпиской не закончилась за 180 с"; bad=1
    fi
fi

echo
echo "журнал движка:"
tail -40 "$log" 2>/dev/null || true
cp "$log" "$root/dist/ios-sim-service.log" 2>/dev/null || true
exit $bad
